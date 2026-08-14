use std::{
    cell::RefCell,
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Barrier,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

mod path_scope {
    pub(crate) use roml::io::mps::{
        MpsDestinationPolicy, MpsPathStage, MpsWriteContext, MpsWriteError, MpsWriteErrorKind,
    };

    pub(crate) mod implementation {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/io/mps/write/path.rs"
        ));
    }
}

use path_scope::{
    implementation::{commit_bytes, commit_bytes_with_ops, MpsPathOps},
    MpsDestinationPolicy, MpsPathStage, MpsWriteError, MpsWriteErrorKind,
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

fn test_directory() -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "roml-mps-path-test-{}-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    ));
    fs::create_dir(&directory).expect("test directory should be new");
    directory
}

fn remove_test_directory(directory: &Path) {
    fs::remove_dir_all(directory).expect("test directory should be removable");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureStage {
    CreateTemp,
    Write,
    Flush,
    Sync,
    Replace,
}

#[derive(Debug)]
struct FakeTemp {
    id: usize,
    path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    CreateTemp(PathBuf),
    TempCreated(PathBuf),
    Write(usize, usize),
    Flush(usize),
    Sync(usize),
    Commit {
        id: usize,
        staged_path: PathBuf,
        destination: PathBuf,
        policy: MpsDestinationPolicy,
    },
    Cleanup(usize),
}

struct MockOps {
    failure: Option<FailureStage>,
    cleanup_failure: bool,
    events: RefCell<Vec<Event>>,
    next_id: AtomicUsize,
}

impl MockOps {
    fn failing(stage: FailureStage) -> Self {
        Self {
            failure: Some(stage),
            cleanup_failure: false,
            events: RefCell::new(Vec::new()),
            next_id: AtomicUsize::new(0),
        }
    }

    fn with_cleanup_failure(stage: FailureStage) -> Self {
        Self {
            failure: Some(stage),
            cleanup_failure: true,
            events: RefCell::new(Vec::new()),
            next_id: AtomicUsize::new(0),
        }
    }

    fn successful_with_cleanup_failure() -> Self {
        Self {
            failure: None,
            cleanup_failure: true,
            events: RefCell::new(Vec::new()),
            next_id: AtomicUsize::new(0),
        }
    }

    fn error(stage: &str) -> io::Error {
        io::Error::other(format!("injected {stage} failure"))
    }

    fn events(&self) -> Vec<Event> {
        self.events.borrow().to_vec()
    }
}

impl MpsPathOps for MockOps {
    type TempHandle = FakeTemp;

    fn create_temp(&self, destination: &Path) -> io::Result<Self::TempHandle> {
        self.events
            .borrow_mut()
            .push(Event::CreateTemp(destination.to_owned()));
        if self.failure == Some(FailureStage::CreateTemp) {
            return Err(Self::error("create temp"));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let path = destination
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".mock-{id}.tmp"));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        self.events
            .borrow_mut()
            .push(Event::TempCreated(path.clone()));
        Ok(FakeTemp { id, path })
    }

    fn write_all(&self, temp: &mut Self::TempHandle, bytes: &[u8]) -> io::Result<()> {
        self.events
            .borrow_mut()
            .push(Event::Write(temp.id, bytes.len()));
        if self.failure == Some(FailureStage::Write) {
            return Err(Self::error("write"));
        }
        fs::write(&temp.path, bytes)?;
        Ok(())
    }

    fn flush(&self, temp: &mut Self::TempHandle) -> io::Result<()> {
        self.events.borrow_mut().push(Event::Flush(temp.id));
        if self.failure == Some(FailureStage::Flush) {
            return Err(Self::error("flush"));
        }
        Ok(())
    }

    fn sync(&self, temp: &mut Self::TempHandle) -> io::Result<()> {
        self.events.borrow_mut().push(Event::Sync(temp.id));
        if self.failure == Some(FailureStage::Sync) {
            return Err(Self::error("sync"));
        }
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temp.path)?
            .sync_all()?;
        Ok(())
    }

    fn atomic_commit(
        &self,
        temp: &mut Self::TempHandle,
        destination: &Path,
        policy: MpsDestinationPolicy,
    ) -> io::Result<()> {
        self.events.borrow_mut().push(Event::Commit {
            id: temp.id,
            staged_path: temp.path.clone(),
            destination: destination.to_owned(),
            policy,
        });
        if self.failure == Some(FailureStage::Replace) {
            return Err(Self::error("replace"));
        }
        match policy {
            MpsDestinationPolicy::CreateNew => fs::hard_link(&temp.path, destination)?,
            MpsDestinationPolicy::AtomicReplace => fs::rename(&temp.path, destination)?,
        }
        Ok(())
    }

    fn cleanup(&self, temp: Self::TempHandle) -> io::Result<()> {
        self.events.borrow_mut().push(Event::Cleanup(temp.id));
        if self.cleanup_failure {
            return Err(Self::error("cleanup"));
        }
        match fs::remove_file(temp.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn created_temp(events: &[Event]) -> &Path {
    events
        .iter()
        .find_map(|event| match event {
            Event::TempCreated(path) => Some(path.as_path()),
            _ => None,
        })
        .expect("successful temp creation should be recorded")
}

fn assert_destination_and_temp_state(
    events: &[Event],
    destination: &Path,
    expected_destination: &[u8],
    temp_exists: bool,
) {
    assert_eq!(fs::read(destination).unwrap(), expected_destination);
    assert_eq!(created_temp(events).exists(), temp_exists);
}

fn assert_stage(error: &MpsWriteError, stage: MpsPathStage) {
    assert_eq!(error.kind(), &MpsWriteErrorKind::Io);
    assert_eq!(error.context().stage(), Some(stage));
}

#[test]
fn create_temp_failure_is_typed_and_does_not_attempt_cleanup() {
    let directory = test_directory();
    let destination = directory.join("model.mps");
    fs::write(&destination, b"old bytes").unwrap();
    let ops = MockOps::failing(FailureStage::CreateTemp);
    let error = commit_bytes_with_ops(
        &ops,
        b"NAME\n",
        &destination,
        MpsDestinationPolicy::AtomicReplace,
    )
    .expect_err("create-temp failure must reject");

    assert_stage(&error, MpsPathStage::CreateTemp);
    assert_eq!(ops.events(), vec![Event::CreateTemp(destination.clone())]);
    assert_eq!(fs::read(&destination).unwrap(), b"old bytes");
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    remove_test_directory(&directory);
}

#[test]
fn write_flush_sync_and_replace_failures_cleanup_the_stage() {
    for (failure, stage) in [
        (FailureStage::Write, MpsPathStage::Write),
        (FailureStage::Flush, MpsPathStage::Flush),
        (FailureStage::Sync, MpsPathStage::Sync),
        (FailureStage::Replace, MpsPathStage::Replace),
    ] {
        let directory = test_directory();
        let destination = directory.join("model.mps");
        fs::write(&destination, b"old bytes").unwrap();
        let ops = MockOps::failing(failure);
        let error = commit_bytes_with_ops(
            &ops,
            b"NAME\n",
            &destination,
            MpsDestinationPolicy::AtomicReplace,
        )
        .expect_err("injected path failure must reject");

        assert_stage(&error, stage);
        assert!(
            ops.events()
                .iter()
                .any(|event| matches!(event, Event::Cleanup(_))),
            "{stage:?} failure must attempt cleanup"
        );
        assert_destination_and_temp_state(&ops.events(), &destination, b"old bytes", false);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        remove_test_directory(&directory);
    }
}

#[test]
fn cleanup_failure_preserves_the_primary_path_failure() {
    let directory = test_directory();
    let destination = directory.join("model.mps");
    fs::write(&destination, b"old bytes").unwrap();
    let ops = MockOps::with_cleanup_failure(FailureStage::Write);
    let error = commit_bytes_with_ops(
        &ops,
        b"NAME\n",
        &destination,
        MpsDestinationPolicy::AtomicReplace,
    )
    .expect_err("primary failure must reject");

    assert_eq!(error.kind(), &MpsWriteErrorKind::PathTransaction);
    assert_eq!(
        error.primary().unwrap().context().stage(),
        Some(MpsPathStage::Write)
    );
    assert_eq!(
        error.cleanup().unwrap().context().stage(),
        Some(MpsPathStage::Cleanup)
    );
    assert!(error.to_string().contains("injected write failure"));
    assert!(error.to_string().contains("injected cleanup failure"));
    assert_destination_and_temp_state(&ops.events(), &destination, b"old bytes", true);
    remove_test_directory(&directory);
}

#[test]
fn cleanup_failure_after_success_reports_cleanup_only_and_keeps_publication() {
    // Chosen semantics: publication has already succeeded, so the returned
    // error is a standalone Cleanup-stage I/O error. There is no primary
    // operation failure to wrap, and the failed best-effort cleanup leaves
    // the staged inode for caller-visible recovery.
    let directory = test_directory();
    let destination = directory.join("model.mps");
    let ops = MockOps::successful_with_cleanup_failure();
    let error = commit_bytes_with_ops(
        &ops,
        b"new bytes",
        &destination,
        MpsDestinationPolicy::CreateNew,
    )
    .expect_err("cleanup failure after publication must be reported");

    assert_eq!(error.kind(), &MpsWriteErrorKind::Io);
    assert_eq!(error.context().stage(), Some(MpsPathStage::Cleanup));
    assert_eq!(
        error.context().message.as_deref(),
        Some("destination published; temporary cleanup failed")
    );
    assert!(error.primary().is_none());
    assert!(error.cleanup().is_none());
    assert_destination_and_temp_state(&ops.events(), &destination, b"new bytes", true);
    remove_test_directory(&directory);
}

#[test]
fn create_new_never_modifies_an_existing_destination() {
    let directory = test_directory();
    let destination = directory.join("model.mps");
    fs::write(&destination, b"old bytes").unwrap();

    let error = commit_bytes(b"new bytes", &destination, MpsDestinationPolicy::CreateNew)
        .expect_err("existing destination must be rejected");

    assert_eq!(error.kind(), &MpsWriteErrorKind::DestinationExists);
    assert_eq!(error.context().stage(), Some(MpsPathStage::Replace));
    assert_eq!(fs::read(&destination).unwrap(), b"old bytes");
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    remove_test_directory(&directory);
}

#[test]
fn create_new_destination_race_preserves_the_racing_file() {
    struct RaceOps;

    impl MpsPathOps for RaceOps {
        type TempHandle = FakeTemp;

        fn create_temp(&self, destination: &Path) -> io::Result<Self::TempHandle> {
            let path = destination.with_extension("tmp");
            fs::write(&path, b"staged bytes")?;
            Ok(FakeTemp { id: 1, path })
        }

        fn write_all(&self, _: &mut Self::TempHandle, _: &[u8]) -> io::Result<()> {
            Ok(())
        }

        fn flush(&self, _: &mut Self::TempHandle) -> io::Result<()> {
            Ok(())
        }

        fn sync(&self, _: &mut Self::TempHandle) -> io::Result<()> {
            Ok(())
        }

        fn atomic_commit(
            &self,
            _: &mut Self::TempHandle,
            destination: &Path,
            policy: MpsDestinationPolicy,
        ) -> io::Result<()> {
            assert_eq!(policy, MpsDestinationPolicy::CreateNew);
            fs::write(destination, b"racing bytes").unwrap();
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "destination race",
            ))
        }

        fn cleanup(&self, temp: Self::TempHandle) -> io::Result<()> {
            fs::remove_file(temp.path)
        }
    }

    let directory = test_directory();
    let destination = directory.join("model.mps");
    let error = commit_bytes_with_ops(
        &RaceOps,
        b"new bytes",
        &destination,
        MpsDestinationPolicy::CreateNew,
    )
    .expect_err("destination race must reject");

    assert_eq!(error.kind(), &MpsWriteErrorKind::DestinationExists);
    assert_eq!(fs::read(&destination).unwrap(), b"racing bytes");
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    remove_test_directory(&directory);
}

#[test]
fn real_create_new_race_publishes_one_complete_file_and_cleans_all_temps() {
    const WRITERS: usize = 8;
    let directory = test_directory();
    let destination = directory.join("model.mps");
    let start = Arc::new(Barrier::new(WRITERS));
    let mut handles = Vec::with_capacity(WRITERS);

    for index in 0..WRITERS {
        let start = Arc::clone(&start);
        let destination = destination.clone();
        handles.push(thread::spawn(move || {
            let payload = format!("NAME writer-{index}\n").into_bytes();
            start.wait();
            match commit_bytes(&payload, &destination, MpsDestinationPolicy::CreateNew) {
                Ok(()) => Ok(payload),
                Err(error) => Err(error.kind().clone()),
            }
        }));
    }

    let mut winner = None;
    let mut destination_exists = 0;
    for handle in handles {
        match handle.join().expect("race worker should not panic") {
            Ok(payload) => {
                assert!(
                    winner.replace(payload).is_none(),
                    "only one writer may publish"
                );
            }
            Err(MpsWriteErrorKind::DestinationExists) => destination_exists += 1,
            Err(error) => panic!("unexpected CreateNew race result: {error:?}"),
        }
    }

    let winner = winner.expect("one writer must publish");
    assert_eq!(destination_exists, WRITERS - 1);
    assert_eq!(fs::read(&destination).unwrap(), winner);
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    remove_test_directory(&directory);
}

#[test]
fn atomic_replace_uses_one_commit_event_and_stages_in_destination_directory() {
    let directory = test_directory();
    let destination = directory.join("model.mps");
    fs::write(&destination, b"old bytes").unwrap();

    let result = commit_bytes(
        b"new bytes",
        &destination,
        MpsDestinationPolicy::AtomicReplace,
    );

    match result {
        Ok(()) => assert_eq!(fs::read(&destination).unwrap(), b"new bytes"),
        Err(error) => {
            assert_eq!(error.kind(), &MpsWriteErrorKind::AtomicReplaceUnavailable);
            assert_eq!(fs::read(&destination).unwrap(), b"old bytes");
        }
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    remove_test_directory(&directory);
}

#[test]
fn injected_atomic_replace_has_no_remove_then_rename_operation() {
    let directory = test_directory();
    let destination = directory.join("model.mps");
    fs::write(&destination, b"old bytes").unwrap();
    let ops = MockOps::failing(FailureStage::Replace);
    let error = commit_bytes_with_ops(
        &ops,
        b"new bytes",
        &destination,
        MpsDestinationPolicy::AtomicReplace,
    )
    .expect_err("replace failure must reject");

    assert_stage(&error, MpsPathStage::Replace);
    let events = ops.events();
    let commit = events
        .iter()
        .find_map(|event| match event {
            Event::Commit {
                staged_path,
                destination,
                policy,
                ..
            } => Some((staged_path, destination, policy)),
            _ => None,
        })
        .expect("one atomic commit operation should be attempted");
    assert_eq!(commit.2, &MpsDestinationPolicy::AtomicReplace);
    assert_eq!(commit.0.parent(), commit.1.parent());
    assert_destination_and_temp_state(&events, &destination, b"old bytes", false);
    remove_test_directory(&directory);
}

#[test]
fn unavailable_atomic_replace_is_reported_before_destination_change() {
    struct UnsupportedOps;

    impl MpsPathOps for UnsupportedOps {
        type TempHandle = FakeTemp;

        fn create_temp(&self, destination: &Path) -> io::Result<Self::TempHandle> {
            let path = destination.with_extension("tmp");
            fs::write(&path, b"staged bytes")?;
            Ok(FakeTemp { id: 1, path })
        }
        fn write_all(&self, _: &mut Self::TempHandle, _: &[u8]) -> io::Result<()> {
            Ok(())
        }
        fn flush(&self, _: &mut Self::TempHandle) -> io::Result<()> {
            Ok(())
        }
        fn sync(&self, _: &mut Self::TempHandle) -> io::Result<()> {
            Ok(())
        }
        fn atomic_commit(
            &self,
            _: &mut Self::TempHandle,
            _: &Path,
            _: MpsDestinationPolicy,
        ) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "atomic replacement unavailable",
            ))
        }
        fn cleanup(&self, temp: Self::TempHandle) -> io::Result<()> {
            fs::remove_file(temp.path)
        }
    }

    let directory = test_directory();
    let destination = directory.join("model.mps");
    fs::write(&destination, b"old bytes").unwrap();
    let error = commit_bytes_with_ops(
        &UnsupportedOps,
        b"new bytes",
        &destination,
        MpsDestinationPolicy::AtomicReplace,
    )
    .expect_err("unsupported replacement must reject");

    assert_eq!(error.kind(), &MpsWriteErrorKind::AtomicReplaceUnavailable);
    assert_eq!(error.context().stage(), Some(MpsPathStage::Replace));
    assert_eq!(fs::read(&destination).unwrap(), b"old bytes");
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    remove_test_directory(&directory);
}
