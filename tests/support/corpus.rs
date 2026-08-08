//! Test-support infrastructure for optional external MPS corpora.
//!
//! This module deliberately remains under `tests/`: it is qualification
//! infrastructure, not part of ROML's production MPS reader API.
//!
//! ## Archive integration boundary
//!
//! A future qualification-only archive adapter (Task 35-08) must obtain each
//! archive entry's *logical* path and entry kind from an archive listing API,
//! then construct [`ArchiveEntry`] with the original payload reader. It must
//! pass entries to [`materialize_chinneck_archive`] in archive order. This
//! helper validates the path and kind before it creates any destination for
//! that entry; an adapter must never invoke a blind extractor and scan its
//! output afterward. No archive library is selected here because the pinned
//! Chinneck archives require a later, evidence-backed reader choice.

use std::{
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// The immutable source identity for one optional external corpus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CorpusPin {
    /// Stable identifier used by the qualification manifest.
    pub corpus_id: &'static str,
    /// Required `origin` URL for the initialized corpus checkout.
    pub repository: &'static str,
    /// Required immutable commit SHA.
    pub commit: &'static str,
    /// Checkout location relative to the ROML repository root.
    pub relative_checkout: &'static str,
}

/// The reviewed Chinneck corpus pin required by MPS-Q08.
pub const CHINNECK_PIN: CorpusPin = CorpusPin {
    corpus_id: "chinneck-infeasible-lps",
    repository: "https://github.com/sk-surya/infeasiblelps",
    commit: "97a936498e5240d44adaf7dcfe84877fa34ce301",
    relative_checkout: "testdata/corpora/infeasible-lps",
};

/// The reviewed converted-Netlib corpus pin required by MPS-Q08.
pub const NETLIB_PIN: CorpusPin = CorpusPin {
    corpus_id: "netlib-lp-data",
    repository: "https://github.com/sk-surya/lp-data-netlib",
    commit: "56257eea85b433ce6aa67d26156b36385318fd6f",
    relative_checkout: "testdata/corpora/netlib",
};

/// The deterministic corpus-pin manifest consumed by later qualification work.
pub const CORPUS_PINS: [CorpusPin; 2] = [CHINNECK_PIN, NETLIB_PIN];

/// A deterministic external-corpus classification reserved for Task 35-08.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CorpusClassification {
    /// The file is inside the accepted P35 dialect and passed qualification.
    SupportedPass,
    /// The file is outside ROML's accepted dialect for the recorded reason.
    IntentionallyUnsupported { reason: String },
    /// The file was not run for the recorded environmental or tier reason.
    Skipped { reason: String },
    /// The file was run but did not qualify.
    Failed { reason: String },
}

/// A machine-readable manifest row shape for later corpus inventory work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusManifestEntry {
    /// Corpus identity from [`CORPUS_PINS`].
    pub corpus_id: &'static str,
    /// Source archive or source directory inside the pinned checkout.
    pub source_archive_or_directory: String,
    /// Model path relative to that source.
    pub relative_model_path: String,
    /// Deterministic support/qualification disposition.
    pub classification: CorpusClassification,
}

/// Exact-pin validation failure for a present optional checkout.
#[derive(Debug)]
pub enum PinValidationError {
    /// A present checkout did not have the reviewed origin URL.
    RepositoryMismatch {
        /// Corpus being checked.
        corpus_id: &'static str,
        /// Required origin URL.
        expected: &'static str,
        /// Observed origin URL.
        actual: String,
    },
    /// A present checkout did not resolve to the reviewed commit SHA.
    CommitMismatch {
        /// Corpus being checked.
        corpus_id: &'static str,
        /// Required commit SHA.
        expected: &'static str,
        /// Observed commit SHA.
        actual: String,
    },
    /// Git could not provide the required checkout metadata.
    Git {
        /// Checkout queried.
        checkout: PathBuf,
        /// Git operation attempted.
        operation: &'static str,
        /// Actionable command failure detail.
        detail: String,
    },
    /// Exactly one of the two coupled optional checkouts was present.
    IncompleteSetup {
        /// The expected but absent checkout.
        missing: PathBuf,
    },
}

impl fmt::Display for PinValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepositoryMismatch {
                corpus_id,
                expected,
                actual,
            } => write!(
                formatter,
                "optional corpus {corpus_id} has origin {actual:?}; expected {expected:?}"
            ),
            Self::CommitMismatch {
                corpus_id,
                expected,
                actual,
            } => write!(
                formatter,
                "optional corpus {corpus_id} is at {actual:?}; expected {expected:?}"
            ),
            Self::Git {
                checkout,
                operation,
                detail,
            } => write!(
                formatter,
                "cannot {operation} optional corpus checkout {}: {detail}",
                checkout.display()
            ),
            Self::IncompleteSetup { missing } => write!(
                formatter,
                "optional corpus setup is incomplete; initialize {} or remove the partial setup",
                missing.display()
            ),
        }
    }
}

impl Error for PinValidationError {}

/// Verify metadata supplied by a qualification runner against one reviewed pin.
pub fn validate_pin(
    pin: CorpusPin,
    repository: &str,
    commit: &str,
) -> Result<(), PinValidationError> {
    if repository != pin.repository {
        return Err(PinValidationError::RepositoryMismatch {
            corpus_id: pin.corpus_id,
            expected: pin.repository,
            actual: repository.to_owned(),
        });
    }
    if commit != pin.commit {
        return Err(PinValidationError::CommitMismatch {
            corpus_id: pin.corpus_id,
            expected: pin.commit,
            actual: commit.to_owned(),
        });
    }
    Ok(())
}

/// Validate both optional corpus checkouts when they are initialized.
///
/// An entirely absent `testdata/corpora/` returns `Ok(None)`, keeping ordinary
/// test runs independent of external data as required by MPS-Q09. A present
/// checkout is always checked for both its reviewed origin and exact `HEAD`.
pub fn validate_optional_corpora(
    repository_root: &Path,
) -> Result<Option<[PathBuf; 2]>, PinValidationError> {
    let checkouts = CORPUS_PINS.map(|pin| repository_root.join(pin.relative_checkout));
    let present = checkouts.each_ref().map(|checkout| checkout.is_dir());
    if present == [false, false] {
        return Ok(None);
    }
    if !present[0] {
        return Err(PinValidationError::IncompleteSetup {
            missing: checkouts[0].clone(),
        });
    }
    if !present[1] {
        return Err(PinValidationError::IncompleteSetup {
            missing: checkouts[1].clone(),
        });
    }

    for (pin, checkout) in CORPUS_PINS.into_iter().zip(&checkouts) {
        let repository = git_value(checkout, "read origin URL", ["remote", "get-url", "origin"])?;
        let commit = git_value(checkout, "resolve HEAD", ["rev-parse", "HEAD"])?;
        validate_pin(pin, repository.trim(), commit.trim())?;
    }
    Ok(Some(checkouts))
}

fn git_value<const N: usize>(
    checkout: &Path,
    operation: &'static str,
    arguments: [&str; N],
) -> Result<String, PinValidationError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(arguments)
        .output()
        .map_err(|source| PinValidationError::Git {
            checkout: checkout.to_owned(),
            operation,
            detail: source.to_string(),
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }

    Err(PinValidationError::Git {
        checkout: checkout.to_owned(),
        operation,
        detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

/// The archive metadata kinds the materializer accepts or rejects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveEntryKind {
    /// A directory entry.
    Directory,
    /// A regular file entry.
    RegularFile,
    /// A symbolic link entry.
    Symlink,
    /// A hard-link entry.
    Hardlink,
    /// A block-device entry.
    BlockDevice,
    /// A character-device entry.
    CharacterDevice,
    /// A FIFO entry.
    Fifo,
    /// A socket entry.
    Socket,
    /// Any archive-specific special entry not represented above.
    Special,
}

/// One archive entry supplied by a future archive adapter or deterministic test.
pub struct ArchiveEntry {
    logical_path: String,
    kind: ArchiveEntryKind,
    payload: Box<dyn Read>,
}

impl ArchiveEntry {
    /// Construct an entry backed by in-memory bytes for deterministic tests.
    pub fn new(logical_path: impl Into<String>, kind: ArchiveEntryKind, payload: Vec<u8>) -> Self {
        Self::with_reader(logical_path, kind, io::Cursor::new(payload))
    }

    /// Construct a regular-file entry backed by in-memory bytes.
    pub fn regular_file(logical_path: impl Into<String>, payload: Vec<u8>) -> Self {
        Self::new(logical_path, ArchiveEntryKind::RegularFile, payload)
    }

    /// Construct a directory entry.
    pub fn directory(logical_path: impl Into<String>) -> Self {
        Self::new(logical_path, ArchiveEntryKind::Directory, Vec::new())
    }

    /// Construct an entry using the original archive reader for its payload.
    pub fn with_reader(
        logical_path: impl Into<String>,
        kind: ArchiveEntryKind,
        payload: impl Read + 'static,
    ) -> Self {
        Self {
            logical_path: logical_path.into(),
            kind,
            payload: Box::new(payload),
        }
    }

    /// Construct a regular-file entry whose payload read fails deterministically.
    pub fn failing_file(
        logical_path: impl Into<String>,
        kind: io::ErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self::with_reader(
            logical_path,
            ArchiveEntryKind::RegularFile,
            FailingReader {
                kind,
                message: message.into(),
            },
        )
    }
}

struct FailingReader {
    kind: io::ErrorKind,
    message: String,
}

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(self.kind, self.message.clone()))
    }
}

/// A cache identity derived from a source corpus commit and archive hash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusCacheKey {
    directory_name: String,
}

impl CorpusCacheKey {
    /// Create a cache key from an exact corpus SHA, archive identity, and hash.
    pub fn new(
        corpus_commit: &str,
        archive_identity: &str,
        archive_hash: &str,
    ) -> Result<Self, MaterializationError> {
        if !is_ascii_hex(corpus_commit, 40) || !is_ascii_hex(archive_hash, 64) {
            return Err(MaterializationError::InvalidCacheKey {
                reason: "corpus commit must be 40 hex characters and archive hash must be 64 hex characters",
            });
        }
        if archive_identity.is_empty()
            || archive_identity.contains(['/', '\\', '\0'])
            || archive_identity == "."
            || archive_identity == ".."
        {
            return Err(MaterializationError::InvalidCacheKey {
                reason: "archive identity must be one safe file-name component",
            });
        }
        Ok(Self {
            directory_name: format!("{corpus_commit}--{archive_identity}--{archive_hash}"),
        })
    }

    /// The deterministic cache-directory name.
    pub fn directory_name(&self) -> &str {
        &self.directory_name
    }
}

fn is_ascii_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Materialization failure that preserves whether no cache output was promoted.
#[derive(Debug)]
pub enum MaterializationError {
    /// The logical archive path cannot be safely materialized.
    UnsafePath {
        /// Original archive path.
        path: String,
        /// Why the path is unsafe.
        reason: &'static str,
    },
    /// An archive entry has a non-regular/non-directory type.
    UnsafeEntryKind {
        /// Original archive path.
        path: String,
        /// Rejected type.
        kind: ArchiveEntryKind,
    },
    /// A filesystem path component was an existing symlink.
    SymlinkTraversal {
        /// Symlink component that would be traversed.
        path: PathBuf,
    },
    /// The destination did not remain beneath the fresh extraction root.
    EscapingDestination {
        /// Computed destination.
        destination: PathBuf,
    },
    /// A final cache directory exists but lacks a trustworthy completion marker.
    IncompleteCache {
        /// Existing cache path.
        path: PathBuf,
    },
    /// Another materializer currently owns this cache key.
    CacheBusy {
        /// Lock path.
        path: PathBuf,
    },
    /// Cache-key data is malformed.
    InvalidCacheKey {
        /// Validation reason.
        reason: &'static str,
    },
    /// A filesystem or payload operation failed.
    Io {
        /// Operation being attempted.
        operation: &'static str,
        /// Path involved in the failure.
        path: PathBuf,
        /// Original I/O failure.
        source: io::Error,
    },
}

impl fmt::Display for MaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafePath { path, reason } => {
                write!(formatter, "unsafe archive path {path:?}: {reason}")
            }
            Self::UnsafeEntryKind { path, kind } => {
                write!(formatter, "unsafe archive entry kind {kind:?} for {path:?}")
            }
            Self::SymlinkTraversal { path } => {
                write!(
                    formatter,
                    "refusing filesystem symlink traversal at {}",
                    path.display()
                )
            }
            Self::EscapingDestination { destination } => write!(
                formatter,
                "archive destination escapes the fresh extraction root: {}",
                destination.display()
            ),
            Self::IncompleteCache { path } => write!(
                formatter,
                "refusing incomplete corpus cache at {}",
                path.display()
            ),
            Self::CacheBusy { path } => {
                write!(
                    formatter,
                    "corpus cache key is already materializing: {}",
                    path.display()
                )
            }
            Self::InvalidCacheKey { reason } => {
                write!(formatter, "invalid corpus cache key: {reason}")
            }
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
        }
    }
}

impl Error for MaterializationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

const COMPLETION_MARKER: &str = ".roml-corpus-complete";
static STAGING_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Safely materialize one Chinneck archive into a cache directory.
///
/// The returned directory is either a previously completed cache key or a
/// directory atomically promoted from a fresh staging root. Any validation,
/// payload, or filesystem error removes the staging tree and leaves no final
/// cache directory behind.
pub fn materialize_chinneck_archive(
    cache_root: &Path,
    cache_key: &CorpusCacheKey,
    entries: impl IntoIterator<Item = ArchiveEntry>,
) -> Result<PathBuf, MaterializationError> {
    ensure_directory_tree_without_symlinks(cache_root)?;
    let final_cache = cache_root.join(cache_key.directory_name());
    ensure_no_symlink_components(&final_cache)?;

    if final_cache.exists() {
        return completed_cache(&final_cache).map(|()| final_cache);
    }

    let _lock = CacheLock::acquire(cache_root, cache_key)?;
    if final_cache.exists() {
        return completed_cache(&final_cache).map(|()| final_cache);
    }

    let mut staging = FreshStaging::create(cache_root, cache_key)?;
    for mut entry in entries {
        let relative_path = validate_logical_path(&entry.logical_path)?;
        validate_entry_kind(&entry)?;

        let destination = staging.path.join(&relative_path);
        ensure_beneath(&staging.path, &destination)?;
        match entry.kind {
            ArchiveEntryKind::Directory => {
                create_relative_directory(&staging.path, &relative_path)?;
            }
            ArchiveEntryKind::RegularFile => {
                if let Some(parent) = relative_path.parent() {
                    create_relative_directory(&staging.path, parent)?;
                }
                ensure_no_symlink_components(&destination)?;
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&destination)
                    .map_err(|source| io_error("create archive file", &destination, source))?;
                io::copy(&mut entry.payload, &mut output)
                    .map_err(|source| io_error("write archive file", &destination, source))?;
                output
                    .flush()
                    .map_err(|source| io_error("flush archive file", &destination, source))?;
            }
            kind => {
                return Err(MaterializationError::UnsafeEntryKind {
                    path: entry.logical_path,
                    kind,
                });
            }
        }
    }

    let marker = staging.path.join(COMPLETION_MARKER);
    File::create_new(&marker)
        .and_then(|mut file| file.write_all(b"complete\n"))
        .map_err(|source| io_error("write completion marker", &marker, source))?;
    ensure_no_symlink_components(&marker)?;

    fs::rename(&staging.path, &final_cache)
        .map_err(|source| io_error("atomically promote corpus cache", &final_cache, source))?;
    staging.promoted = true;
    Ok(final_cache)
}

fn validate_entry_kind(entry: &ArchiveEntry) -> Result<(), MaterializationError> {
    match entry.kind {
        ArchiveEntryKind::Directory | ArchiveEntryKind::RegularFile => Ok(()),
        kind => Err(MaterializationError::UnsafeEntryKind {
            path: entry.logical_path.clone(),
            kind,
        }),
    }
}

fn validate_logical_path(logical_path: &str) -> Result<PathBuf, MaterializationError> {
    if logical_path.is_empty() || logical_path.contains('\0') {
        return Err(unsafe_path(logical_path, "path is empty or contains NUL"));
    }
    if logical_path.starts_with(['/', '\\']) {
        return Err(unsafe_path(
            logical_path,
            "path is POSIX-absolute or UNC/rooted",
        ));
    }
    let bytes = logical_path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(unsafe_path(logical_path, "path is Windows drive-qualified"));
    }
    if logical_path.contains(':') {
        return Err(unsafe_path(
            logical_path,
            "path contains a Windows device separator",
        ));
    }

    let mut relative = PathBuf::new();
    for component in logical_path.split(['/', '\\']) {
        match component {
            "" | "." => continue,
            ".." => return Err(unsafe_path(logical_path, "path contains lexical traversal")),
            safe => relative.push(safe),
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(unsafe_path(
            logical_path,
            "path resolves to the extraction root",
        ));
    }
    Ok(relative)
}

fn unsafe_path(path: &str, reason: &'static str) -> MaterializationError {
    MaterializationError::UnsafePath {
        path: path.to_owned(),
        reason,
    }
}

fn ensure_beneath(root: &Path, destination: &Path) -> Result<(), MaterializationError> {
    if destination.strip_prefix(root).is_ok() {
        Ok(())
    } else {
        Err(MaterializationError::EscapingDestination {
            destination: destination.to_owned(),
        })
    }
}

fn create_relative_directory(root: &Path, relative: &Path) -> Result<(), MaterializationError> {
    let mut current = root.to_owned();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(MaterializationError::EscapingDestination {
                destination: root.join(relative),
            });
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(MaterializationError::SymlinkTraversal { path: current });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io_error(
                    "create archive directory",
                    &current,
                    io::Error::new(io::ErrorKind::AlreadyExists, "path is not a directory"),
                ));
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|source| io_error("create archive directory", &current, source))?;
            }
            Err(source) => return Err(io_error("inspect archive directory", &current, source)),
        }
    }
    ensure_no_symlink_components(&current)
}

fn completed_cache(cache: &Path) -> Result<(), MaterializationError> {
    let metadata = fs::symlink_metadata(cache)
        .map_err(|source| io_error("inspect corpus cache", cache, source))?;
    if metadata.file_type().is_symlink() {
        return Err(MaterializationError::SymlinkTraversal {
            path: cache.to_owned(),
        });
    }
    if !metadata.is_dir() {
        return Err(MaterializationError::IncompleteCache {
            path: cache.to_owned(),
        });
    }
    let marker = cache.join(COMPLETION_MARKER);
    let marker_metadata =
        fs::symlink_metadata(&marker).map_err(|_| MaterializationError::IncompleteCache {
            path: cache.to_owned(),
        })?;
    if marker_metadata.file_type().is_symlink() || !marker_metadata.is_file() {
        return Err(MaterializationError::IncompleteCache {
            path: cache.to_owned(),
        });
    }
    ensure_no_symlink_components(&marker)
}

fn ensure_directory_tree_without_symlinks(path: &Path) -> Result<(), MaterializationError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err(MaterializationError::UnsafePath {
                    path: path.display().to_string(),
                    reason: "cache root contains lexical traversal",
                });
            }
            Component::Normal(part) => current.push(part),
        }
        if current.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(MaterializationError::SymlinkTraversal { path: current });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io_error(
                    "create corpus cache root",
                    &current,
                    io::Error::new(io::ErrorKind::AlreadyExists, "path is not a directory"),
                ));
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|source| io_error("create corpus cache root", &current, source))?;
            }
            Err(source) => return Err(io_error("inspect corpus cache root", &current, source)),
        }
    }
    Ok(())
}

fn ensure_no_symlink_components(path: &Path) -> Result<(), MaterializationError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err(MaterializationError::UnsafePath {
                    path: path.display().to_string(),
                    reason: "filesystem destination contains lexical traversal",
                });
            }
            Component::Normal(part) => current.push(part),
        }
        if current.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(MaterializationError::SymlinkTraversal { path: current });
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => break,
            Err(source) => return Err(io_error("inspect filesystem path", &current, source)),
        }
    }
    Ok(())
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> MaterializationError {
    MaterializationError::Io {
        operation,
        path: path.to_owned(),
        source,
    }
}

struct FreshStaging {
    path: PathBuf,
    promoted: bool,
}

impl FreshStaging {
    fn create(cache_root: &Path, cache_key: &CorpusCacheKey) -> Result<Self, MaterializationError> {
        for _ in 0..64 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|source| {
                    io_error("read system time", cache_root, io::Error::other(source))
                })?
                .as_nanos();
            let path = cache_root.join(format!(
                ".{}--staging-{}-{nanos}-{sequence}",
                cache_key.directory_name(),
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        promoted: false,
                    })
                }
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => return Err(io_error("create fresh extraction root", &path, source)),
            }
        }
        Err(MaterializationError::CacheBusy {
            path: cache_root.join(format!(".{}--staging", cache_key.directory_name())),
        })
    }
}

impl Drop for FreshStaging {
    fn drop(&mut self) {
        if !self.promoted {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

struct CacheLock {
    path: PathBuf,
}

impl CacheLock {
    fn acquire(
        cache_root: &Path,
        cache_key: &CorpusCacheKey,
    ) -> Result<Self, MaterializationError> {
        let path = cache_root.join(format!(".{}--lock", cache_key.directory_name()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => Ok(Self { path }),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                Err(MaterializationError::CacheBusy { path })
            }
            Err(source) => Err(io_error("acquire corpus cache lock", &path, source)),
        }
    }
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
