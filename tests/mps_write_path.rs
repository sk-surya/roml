use std::{
    cell::RefCell,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
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
        "roml-mps-path-test-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
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
        Ok(FakeTemp {
            id,
            path: destination
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(format!(".mock-{id}.tmp")),
        })
    }

    fn write_all(&self, temp: &mut Self::TempHandle, bytes: &[u8]) -> io::Result<()> {
        self.events
            .borrow_mut()
            .push(Event::Write(temp.id, bytes.len()));
        if self.failure == Some(FailureStage::Write) {
            return Err(Self::error("write"));
        }
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
        Ok(())
    }

    fn cleanup(&self, temp: Self::TempHandle) -> io::Result<()> {
        self.events.borrow_mut().push(Event::Cleanup(temp.id));
        if self.cleanup_failure {
            return Err(Self::error("cleanup"));
        }
        Ok(())
    }
}

fn assert_stage(error: &MpsWriteError, stage: MpsPathStage) {
    assert_eq!(error.kind(), &MpsWriteErrorKind::Io);
    assert_eq!(error.context().stage(), Some(stage));
}

#[test]
fn create_temp_failure_is_typed_and_does_not_attempt_cleanup() {
    let ops = MockOps::failing(FailureStage::CreateTemp);
    let error = commit_bytes_with_ops(
        &ops,
        b"NAME\n",
        Path::new("out/model.mps"),
        MpsDestinationPolicy::AtomicReplace,
    )
    .expect_err("create-temp failure must reject");

    assert_stage(&error, MpsPathStage::CreateTemp);
    assert_eq!(
        ops.events(),
        vec![Event::CreateTemp(PathBuf::from("out/model.mps"))]
    );
}

#[test]
fn write_flush_sync_and_replace_failures_cleanup_the_stage() {
    for (failure, stage) in [
        (FailureStage::Write, MpsPathStage::Write),
        (FailureStage::Flush, MpsPathStage::Flush),
        (FailureStage::Sync, MpsPathStage::Sync),
        (FailureStage::Replace, MpsPathStage::Replace),
    ] {
        let ops = MockOps::failing(failure);
        let error = commit_bytes_with_ops(
            &ops,
            b"NAME\n",
            Path::new("out/model.mps"),
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
    }
}

#[test]
fn cleanup_failure_preserves_the_primary_path_failure() {
    let ops = MockOps::with_cleanup_failure(FailureStage::Write);
    let error = commit_bytes_with_ops(
        &ops,
        b"NAME\n",
        Path::new("out/model.mps"),
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
            Ok(FakeTemp {
                id: 1,
                path: destination.with_extension("tmp"),
            })
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

        fn cleanup(&self, _: Self::TempHandle) -> io::Result<()> {
            Ok(())
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
    let ops = MockOps::failing(FailureStage::Replace);
    let error = commit_bytes_with_ops(
        &ops,
        b"new bytes",
        Path::new("out/model.mps"),
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
}

#[test]
fn unavailable_atomic_replace_is_reported_before_destination_change() {
    struct UnsupportedOps;

    impl MpsPathOps for UnsupportedOps {
        type TempHandle = FakeTemp;

        fn create_temp(&self, destination: &Path) -> io::Result<Self::TempHandle> {
            Ok(FakeTemp {
                id: 1,
                path: destination.with_extension("tmp"),
            })
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
        fn cleanup(&self, _: Self::TempHandle) -> io::Result<()> {
            Ok(())
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
    remove_test_directory(&directory);
}
