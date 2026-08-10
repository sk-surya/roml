// Transactional filesystem commit for MPS bytes.
//
// This module deliberately knows nothing about model projection or MPS
// formatting. It accepts already validated bytes and only owns staging,
// durability, destination policy, and failure cleanup.

use super::{
    MpsDestinationPolicy, MpsPathStage, MpsWriteContext, MpsWriteError, MpsWriteErrorKind,
};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// The testable filesystem operations used by a path transaction.
///
/// The associated handle keeps the seam useful for fault injection without
/// imposing the real filesystem's `File` representation on tests. `flush`
/// and `sync` are separate so the public error context can identify the exact
/// durability stage that failed.
pub(crate) trait MpsPathOps {
    type TempHandle;

    fn create_temp(&self, destination: &Path) -> io::Result<Self::TempHandle>;
    fn write_all(&self, temp: &mut Self::TempHandle, bytes: &[u8]) -> io::Result<()>;
    fn flush(&self, temp: &mut Self::TempHandle) -> io::Result<()>;
    fn sync(&self, temp: &mut Self::TempHandle) -> io::Result<()>;
    fn atomic_commit(
        &self,
        temp: &mut Self::TempHandle,
        destination: &Path,
        policy: MpsDestinationPolicy,
    ) -> io::Result<()>;
    fn cleanup(&self, temp: Self::TempHandle) -> io::Result<()>;

    /// Convenience form matching the conceptual path-ops contract.
    #[allow(dead_code)]
    fn flush_and_sync(&self, temp: &mut Self::TempHandle) -> io::Result<()> {
        self.flush(temp)?;
        self.sync(temp)
    }
}

/// Commits already formatted MPS bytes using the real filesystem.
pub(crate) fn commit_path(
    bytes: &[u8],
    destination: &Path,
    policy: MpsDestinationPolicy,
) -> Result<(), MpsWriteError> {
    commit_path_with_ops(&StdPathOps, bytes, destination, policy)
}

/// Compatibility name for the bytes-only integration seam.
#[allow(dead_code)]
pub(crate) fn commit_bytes(
    bytes: &[u8],
    destination: &Path,
    policy: MpsDestinationPolicy,
) -> Result<(), MpsWriteError> {
    commit_path(bytes, destination, policy)
}

/// Test hook for exercising every path-transaction stage without model or
/// formatter integration.
pub(crate) fn commit_path_with_ops<O: MpsPathOps>(
    ops: &O,
    bytes: &[u8],
    destination: &Path,
    policy: MpsDestinationPolicy,
) -> Result<(), MpsWriteError> {
    let mut temp = match ops.create_temp(destination) {
        Ok(temp) => temp,
        Err(error) => return Err(io_error(destination, MpsPathStage::CreateTemp, error)),
    };

    if let Err(error) = ops.write_all(&mut temp, bytes) {
        return Err(fail_with_cleanup(
            ops,
            temp,
            io_error(destination, MpsPathStage::Write, error),
            destination,
        ));
    }
    if let Err(error) = ops.flush(&mut temp) {
        return Err(fail_with_cleanup(
            ops,
            temp,
            io_error(destination, MpsPathStage::Flush, error),
            destination,
        ));
    }
    if let Err(error) = ops.sync(&mut temp) {
        return Err(fail_with_cleanup(
            ops,
            temp,
            io_error(destination, MpsPathStage::Sync, error),
            destination,
        ));
    }

    match ops.atomic_commit(&mut temp, destination, policy) {
        Ok(()) => {
            // Publication is complete before cleanup runs. A cleanup failure
            // therefore has no primary operation to compose and is reported
            // as a standalone Cleanup-stage error; callers must not mistake
            // this error for an unpublished destination.
            match ops.cleanup(temp) {
                Ok(()) => Ok(()),
                Err(error) => Err(published_cleanup_error(destination, error)),
            }
        }
        Err(error) => {
            let kind = match (policy, error.kind()) {
                (MpsDestinationPolicy::CreateNew, io::ErrorKind::AlreadyExists) => {
                    MpsWriteErrorKind::DestinationExists
                }
                (MpsDestinationPolicy::AtomicReplace, io::ErrorKind::Unsupported) => {
                    MpsWriteErrorKind::AtomicReplaceUnavailable
                }
                _ => MpsWriteErrorKind::Io,
            };
            let primary = MpsWriteError::with_source(
                kind,
                MpsWriteContext::default()
                    .with_path(destination.to_owned())
                    .with_stage(MpsPathStage::Replace),
                error,
            );
            Err(fail_with_cleanup(ops, temp, primary, destination))
        }
    }
}

/// Test hook counterpart for [`commit_bytes`].
#[allow(dead_code)]
pub(crate) fn commit_bytes_with_ops<O: MpsPathOps>(
    ops: &O,
    bytes: &[u8],
    destination: &Path,
    policy: MpsDestinationPolicy,
) -> Result<(), MpsWriteError> {
    commit_path_with_ops(ops, bytes, destination, policy)
}

fn fail_with_cleanup<O: MpsPathOps>(
    ops: &O,
    temp: O::TempHandle,
    primary: MpsWriteError,
    destination: &Path,
) -> MpsWriteError {
    match ops.cleanup(temp) {
        Ok(()) => primary,
        Err(error) => primary.with_cleanup(io_error(destination, MpsPathStage::Cleanup, error)),
    }
}

fn io_error(destination: &Path, stage: MpsPathStage, error: io::Error) -> MpsWriteError {
    MpsWriteError::io(
        MpsWriteContext::default()
            .with_path(destination.to_owned())
            .with_stage(stage),
        error,
    )
}

fn published_cleanup_error(destination: &Path, error: io::Error) -> MpsWriteError {
    MpsWriteError::io(
        MpsWriteContext::default()
            .with_path(destination.to_owned())
            .with_stage(MpsPathStage::Cleanup)
            .with_message("destination published; temporary cleanup failed"),
        error,
    )
}

struct StdPathOps;

struct FileTemp {
    file: File,
    path: PathBuf,
}

impl MpsPathOps for StdPathOps {
    type TempHandle = FileTemp;

    fn create_temp(&self, destination: &Path) -> io::Result<Self::TempHandle> {
        let directory = destination.parent().unwrap_or_else(|| Path::new("."));
        let filename = destination.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "destination has no filename")
        })?;

        for _ in 0..1024 {
            let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let mut temporary_name = OsString::from(".");
            temporary_name.push(filename);
            temporary_name.push(format!(".roml-mps-{}-{sequence}.tmp", std::process::id()));
            let path = directory.join(temporary_name);
            let result = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path);
            match result {
                Ok(file) => return Ok(FileTemp { file, path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to allocate a unique MPS temporary file",
        ))
    }

    fn write_all(&self, temp: &mut Self::TempHandle, bytes: &[u8]) -> io::Result<()> {
        temp.file.write_all(bytes)
    }

    fn flush(&self, temp: &mut Self::TempHandle) -> io::Result<()> {
        temp.file.flush()
    }

    fn sync(&self, temp: &mut Self::TempHandle) -> io::Result<()> {
        temp.file.sync_all()
    }

    fn atomic_commit(
        &self,
        temp: &mut Self::TempHandle,
        destination: &Path,
        policy: MpsDestinationPolicy,
    ) -> io::Result<()> {
        match policy {
            MpsDestinationPolicy::CreateNew => {
                // A hard link is an atomic create-if-absent operation on the
                // same filesystem. Unlike remove-then-rename, it cannot
                // replace a destination that won a concurrent race.
                fs::hard_link(&temp.path, destination)
            }
            MpsDestinationPolicy::AtomicReplace => platform_atomic_replace(&temp.path, destination),
        }
    }

    fn cleanup(&self, temp: Self::TempHandle) -> io::Result<()> {
        match fs::remove_file(temp.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

#[cfg(unix)]
fn platform_atomic_replace(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn platform_atomic_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let mut source: Vec<u16> = source.as_os_str().encode_wide().collect();
    let mut destination: Vec<u16> = destination.as_os_str().encode_wide().collect();
    if source.contains(&0) || destination.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows paths cannot contain NUL characters",
        ));
    }
    source.push(0);
    destination.push(0);

    // MoveFileExW with REPLACE_EXISTING performs one same-volume replacement
    // operation; it does not expose a remove-then-rename sequence.
    // SAFETY: both paths are converted to NUL-terminated UTF-16 buffers that
    // remain alive for the duration of the call. Interior NULs are rejected
    // before the buffers are passed to the generated Windows API.
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn platform_atomic_replace(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic replacement is unavailable on this target",
    ))
}

#[cfg(all(test, windows))]
mod windows_binding_compile_tests {
    #[test]
    fn generated_move_file_ex_api_is_available() {
        use windows_sys::{
            core::{BOOL, PCWSTR},
            Win32::Storage::FileSystem::{
                MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MOVE_FILE_FLAGS,
            },
        };

        let _: unsafe extern "system" fn(PCWSTR, PCWSTR, MOVE_FILE_FLAGS) -> BOOL = MoveFileExW;
        let _: MOVE_FILE_FLAGS = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
    }
}
