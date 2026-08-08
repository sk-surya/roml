//! Test-support infrastructure for optional external MPS corpora.
//!
//! This module deliberately remains under `tests/`: it is qualification
//! infrastructure, not part of ROML's production MPS reader API.
//!
//! ## Archive integration boundary
//!
//! This is a **contract fixture**, not an archive reader or extractor. A
//! future qualification-only archive adapter (Task 35-08) must obtain each
//! archive entry's *logical* path and entry kind from an archive listing API,
//! then construct [`ArchiveEntry`] with the original payload reader. It must
//! pass entries to [`materialize_chinneck_archive`] in archive order. This
//! helper validates the path and kind before it creates any destination for
//! that entry; an adapter must never invoke a blind extractor and scan its
//! output afterward. No archive library is selected or exercised here, so this
//! fixture makes no claim that it extracts a real archive. The pinned Chinneck
//! archives require a later, evidence-backed reader choice.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
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
    /// A checkout has uncommitted, untracked, or conflicted state.
    DirtyCheckout {
        /// Corpus being checked.
        corpus_id: &'static str,
        /// Checkout path.
        checkout: PathBuf,
        /// `git status --porcelain=v1 --untracked-files=all` output.
        status: String,
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
            Self::DirtyCheckout {
                corpus_id,
                checkout,
                status,
            } => write!(
                formatter,
                "optional corpus {corpus_id} checkout {} is not clean: {status:?}",
                checkout.display()
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

/// Validate reviewed metadata plus a clean nested-checkout state.
pub fn validate_pin_checkout(
    pin: CorpusPin,
    checkout: &Path,
    repository: &str,
    commit: &str,
    porcelain_status: &str,
) -> Result<(), PinValidationError> {
    validate_pin(pin, repository, commit)?;
    if !porcelain_status.is_empty() {
        return Err(PinValidationError::DirtyCheckout {
            corpus_id: pin.corpus_id,
            checkout: checkout.to_owned(),
            status: porcelain_status.to_owned(),
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
        let status = git_value(
            checkout,
            "check checkout cleanliness",
            ["status", "--porcelain=v1", "--untracked-files=all"],
        )?;
        validate_pin_checkout(pin, checkout, repository.trim(), commit.trim(), &status)?;
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
            return Err(MaterializationError::InvalidCacheKey { reason: "corpus commit must be 40 hex characters and archive hash must be 64 hex characters" });
        }
        validate_cache_name_component(archive_identity).map_err(|_| {
            MaterializationError::InvalidCacheKey {
                reason: "archive identity must be one portable file-name component",
            }
        })?;
        Ok(Self {
            directory_name: format!("{corpus_commit}--{archive_identity}--{archive_hash}"),
        })
    }

    /// The deterministic cache-directory name.
    pub fn directory_name(&self) -> &str {
        &self.directory_name
    }
}

/// The complete, independent file inventory expected from one archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedArchiveInventory {
    files: BTreeMap<String, String>,
}

impl ExpectedArchiveInventory {
    /// Validate an exact path-to-lowercase-SHA-256 inventory before extraction.
    pub fn new(
        entries: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, MaterializationError> {
        let mut files = BTreeMap::new();
        for (path, digest) in entries {
            validate_logical_path(&path)?;
            if path == COMPLETION_MARKER
                || !is_ascii_hex(&digest, 64)
                || digest.bytes().any(|byte| byte.is_ascii_uppercase())
            {
                return Err(MaterializationError::InvalidInventory {
                    reason:
                        "inventory paths must be portable and digests must be lowercase SHA-256",
                });
            }
            if files.insert(path, digest).is_some() {
                return Err(MaterializationError::InvalidInventory {
                    reason: "inventory contains a duplicate path",
                });
            }
        }
        Ok(Self { files })
    }

    /// Construct an empty expected inventory for rejection-only fixtures.
    pub fn empty() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    fn marker_contents(&self) -> String {
        self.files
            .iter()
            .map(|(path, digest)| format!("{digest}  {path}\n"))
            .collect()
    }
}

fn is_ascii_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Cleanup error retained alongside the primary materialization failure.
#[derive(Debug)]
pub struct CleanupFailure {
    operation: &'static str,
    path: PathBuf,
    source: io::Error,
}

/// Materialization failure. Cleanup and lock-release failures are never lost.
#[derive(Debug)]
pub enum MaterializationError {
    /// The logical archive path cannot be safely materialized.
    UnsafePath { path: String, reason: &'static str },
    /// An archive entry has a non-regular/non-directory type.
    UnsafeEntryKind {
        path: String,
        kind: ArchiveEntryKind,
    },
    /// A no-follow descriptor open encountered a symlink.
    SymlinkTraversal { path: PathBuf },
    /// An expected inventory was malformed.
    InvalidInventory { reason: &'static str },
    /// Observed files or their SHA-256 values differ from the expected inventory.
    InventoryMismatch { reason: String },
    /// A final cache directory is missing a valid completion record.
    IncompleteCache { path: PathBuf },
    /// Another materializer owns this cache key.
    CacheBusy { path: PathBuf },
    /// Cache-key data is malformed.
    InvalidCacheKey { reason: &'static str },
    /// This contract fixture has no race-safe implementation for this platform.
    #[allow(dead_code)]
    UnsupportedPlatform { platform: &'static str },
    /// A filesystem or payload operation failed.
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    /// The original extraction failure plus failures while releasing its resources.
    WithCleanup {
        primary: Box<MaterializationError>,
        cleanup: Vec<CleanupFailure>,
    },
}

impl fmt::Display for MaterializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafePath { path, reason } => write!(f, "unsafe archive path {path:?}: {reason}"),
            Self::UnsafeEntryKind { path, kind } => write!(f, "unsafe archive entry kind {kind:?} for {path:?}"),
            Self::SymlinkTraversal { path } => write!(f, "refusing filesystem symlink traversal at {}", path.display()),
            Self::InvalidInventory { reason } => write!(f, "invalid expected archive inventory: {reason}"),
            Self::InventoryMismatch { reason } => write!(f, "archive inventory does not match expectation: {reason}"),
            Self::IncompleteCache { path } => write!(f, "refusing incomplete corpus cache at {}", path.display()),
            Self::CacheBusy { path } => write!(f, "corpus cache key is already materializing: {}", path.display()),
            Self::InvalidCacheKey { reason } => write!(f, "invalid corpus cache key: {reason}"),
            Self::UnsupportedPlatform { platform } => write!(f, "the corpus contract fixture has no verified no-follow implementation on {platform}"),
            Self::Io { operation, path, source } => write!(f, "cannot {operation} {}: {source}", path.display()),
            Self::WithCleanup { primary, cleanup } => write!(f, "{primary}; additionally failed to clean up {} resource(s)", cleanup.len()),
        }
    }
}

impl Error for MaterializationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::WithCleanup { primary, .. } => Some(primary),
            _ => None,
        }
    }
}

const COMPLETION_MARKER: &str = ".roml-corpus-complete";
static STAGING_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Materialize a reader-supplied entry stream only through the Linux
/// descriptor-relative `openat`/`mkdirat` + `O_NOFOLLOW` contract seam.
///
/// This is deliberately not an archive extractor: callers must supply entry
/// metadata and streams from a separately qualified archive reader.
#[cfg(target_os = "linux")]
pub fn materialize_chinneck_archive(
    cache_root: &Path,
    cache_key: &CorpusCacheKey,
    entries: impl IntoIterator<Item = ArchiveEntry>,
    expected: &ExpectedArchiveInventory,
) -> Result<PathBuf, MaterializationError> {
    materialize_linux(cache_root, cache_key, entries, expected)
}

/// See the Linux implementation for the verified platform contract.
#[cfg(not(target_os = "linux"))]
pub fn materialize_chinneck_archive(
    _cache_root: &Path,
    _cache_key: &CorpusCacheKey,
    _entries: impl IntoIterator<Item = ArchiveEntry>,
    _expected: &ExpectedArchiveInventory,
) -> Result<PathBuf, MaterializationError> {
    Err(MaterializationError::UnsupportedPlatform {
        platform: std::env::consts::OS,
    })
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

fn validate_logical_path(path: &str) -> Result<Vec<&str>, MaterializationError> {
    if path.is_empty() || path.contains('\0') || path.starts_with(['/', '\\']) {
        return Err(unsafe_path(
            path,
            "path is empty, contains NUL, or is rooted",
        ));
    }
    let bytes = path.as_bytes();
    if (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        || path.contains(':')
    {
        return Err(unsafe_path(
            path,
            "path is Windows drive-qualified or contains a device separator",
        ));
    }
    let components: Vec<_> = path.split(['/', '\\']).collect();
    if components
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(unsafe_path(
            path,
            "path contains an empty, dot, or traversal component",
        ));
    }
    for component in &components {
        validate_component(component, path)?;
    }
    Ok(components)
}

fn validate_component(component: &str, original_path: &str) -> Result<(), MaterializationError> {
    if component.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
    }) {
        return Err(unsafe_path(
            original_path,
            "component contains a Windows-invalid or control character",
        ));
    }
    if component.ends_with(['.', ' ']) {
        return Err(unsafe_path(
            original_path,
            "component has a Windows-trimmed trailing dot or space",
        ));
    }
    let device = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(
        device.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "COM¹" | "COM²" | "COM³" | "LPT¹" | "LPT²" | "LPT³"
    ) || device
        .strip_prefix("COM")
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=9).contains(&n))
        || device
            .strip_prefix("LPT")
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=9).contains(&n));
    if reserved {
        return Err(unsafe_path(
            original_path,
            "component is a Windows reserved device name",
        ));
    }
    Ok(())
}

fn validate_cache_name_component(component: &str) -> Result<(), MaterializationError> {
    if component.is_empty() || matches!(component, "." | "..") || component.contains(['/', '\\']) {
        return Err(unsafe_path(
            component,
            "archive identity must be exactly one non-dot cache-name component",
        ));
    }
    validate_component(component, component)
}

fn unsafe_path(path: &str, reason: &'static str) -> MaterializationError {
    MaterializationError::UnsafePath {
        path: path.to_owned(),
        reason,
    }
}

fn io_error(
    operation: &'static str,
    path: &Path,
    source: impl Into<io::Error>,
) -> MaterializationError {
    MaterializationError::Io {
        operation,
        path: path.to_owned(),
        source: source.into(),
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use rustix::{
        fs::{self as rfs, AtFlags, FileType, Mode, OFlags, RawDir, RenameFlags},
        io::Errno,
    };
    use sha2::{Digest, Sha256};
    use std::{ffi::OsStr, mem::MaybeUninit, path::Component};

    fn dir_flags() -> OFlags {
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
    }
    fn file_flags() -> OFlags {
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC
    }

    pub(super) fn materialize_linux(
        cache_root: &Path,
        cache_key: &CorpusCacheKey,
        entries: impl IntoIterator<Item = ArchiveEntry>,
        expected: &ExpectedArchiveInventory,
    ) -> Result<PathBuf, MaterializationError> {
        let root = SecureDirectory::open_or_create(cache_root)?;
        let mut lock = CacheLock::acquire(&root, cache_key)?;

        if let Some(cache) = root.open_existing_dir(cache_key.directory_name())? {
            let result = validate_cache(&cache, expected).map(|()| cache.path.clone());
            return finish_without_staging(result, &mut lock);
        }

        let mut staging = FreshStaging::create(&root, cache_key)?;
        let result = extract(&mut staging, entries, expected)
            .and_then(|()| root.promote_noreplace(&staging.name, cache_key.directory_name()))
            .map(|()| {
                staging.promoted = true;
                root.path.join(cache_key.directory_name())
            });
        finish_with_staging(result, &mut staging, &mut lock)
    }

    fn finish_without_staging<T>(
        result: Result<T, MaterializationError>,
        lock: &mut CacheLock,
    ) -> Result<T, MaterializationError> {
        match result {
            Ok(value) => {
                lock.release().map_err(cleanup_error)?;
                Ok(value)
            }
            Err(primary) => Err(with_cleanup(primary, Vec::new(), lock)),
        }
    }

    fn finish_with_staging<T>(
        result: Result<T, MaterializationError>,
        staging: &mut FreshStaging,
        lock: &mut CacheLock,
    ) -> Result<T, MaterializationError> {
        match result {
            Ok(value) => {
                lock.release().map_err(cleanup_error)?;
                Ok(value)
            }
            Err(primary) => {
                let mut cleanup = Vec::new();
                staging.cleanup(&mut cleanup);
                Err(with_cleanup(primary, cleanup, lock))
            }
        }
    }

    fn with_cleanup(
        primary: MaterializationError,
        mut cleanup: Vec<CleanupFailure>,
        lock: &mut CacheLock,
    ) -> MaterializationError {
        if let Err(failure) = lock.release() {
            cleanup.push(failure);
        }
        if cleanup.is_empty() {
            primary
        } else {
            MaterializationError::WithCleanup {
                primary: Box::new(primary),
                cleanup,
            }
        }
    }

    fn cleanup_error(failure: CleanupFailure) -> MaterializationError {
        MaterializationError::Io {
            operation: failure.operation,
            path: failure.path,
            source: failure.source,
        }
    }

    fn extract(
        staging: &mut FreshStaging,
        entries: impl IntoIterator<Item = ArchiveEntry>,
        expected: &ExpectedArchiveInventory,
    ) -> Result<(), MaterializationError> {
        let mut observed = BTreeMap::new();
        for mut entry in entries {
            validate_entry_kind(&entry)?;
            let components = validate_logical_path(&entry.logical_path)?;
            match entry.kind {
                ArchiveEntryKind::Directory => staging.dir.create_relative_dir(&components)?,
                ArchiveEntryKind::RegularFile => {
                    let digest = staging
                        .dir
                        .write_relative_file(&components, &mut entry.payload)?;
                    if observed
                        .insert(entry.logical_path.clone(), digest)
                        .is_some()
                    {
                        return Err(MaterializationError::InventoryMismatch {
                            reason: format!("duplicate file entry {:?}", entry.logical_path),
                        });
                    }
                }
                _ => unreachable!("entry kind was validated"),
            }
        }
        if observed != expected.files {
            return Err(MaterializationError::InventoryMismatch {
                reason: "observed paths or SHA-256 values differ".to_owned(),
            });
        }
        staging.dir.write_relative_file(
            &[COMPLETION_MARKER],
            &mut io::Cursor::new(expected.marker_contents().into_bytes()),
        )?;
        Ok(())
    }

    fn validate_cache(
        cache: &SecureDirectory,
        expected: &ExpectedArchiveInventory,
    ) -> Result<(), MaterializationError> {
        let marker = cache
            .read_relative_file(&[COMPLETION_MARKER])
            .map_err(|_| MaterializationError::IncompleteCache {
                path: cache.path.clone(),
            })?;
        if marker != expected.marker_contents().as_bytes() {
            return Err(MaterializationError::InventoryMismatch {
                reason: "completion inventory differs from expected inventory".to_owned(),
            });
        }
        let mut observed = BTreeMap::new();
        collect_cache_files(cache, "", &mut observed)?;
        if observed != expected.files {
            return Err(MaterializationError::InventoryMismatch {
                reason: "cached files or SHA-256 values differ from expected inventory".to_owned(),
            });
        }
        Ok(())
    }

    fn collect_cache_files(
        directory: &SecureDirectory,
        prefix: &str,
        observed: &mut BTreeMap<String, String>,
    ) -> Result<(), MaterializationError> {
        let fd = directory.clone_open()?;
        let mut buffer = [MaybeUninit::uninit(); 8192];
        let mut entries = RawDir::new(&fd.fd, &mut buffer);
        while let Some(entry) = entries.next() {
            let entry = entry
                .map_err(|source| io_error("enumerate cache inventory", &directory.path, source))?;
            let name = entry
                .file_name()
                .to_str()
                .map_err(|_| MaterializationError::InventoryMismatch {
                    reason: "cached path is not valid UTF-8".to_owned(),
                })?
                .to_owned();
            if name == "." || name == ".." {
                continue;
            }
            let logical_path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if logical_path == COMPLETION_MARKER {
                continue;
            }
            let stat =
                rfs::statat(&directory.fd, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|source| {
                    io_error(
                        "inspect cache entry without following links",
                        &directory.path.join(&name),
                        source,
                    )
                })?;
            let file_type = FileType::from_raw_mode(stat.st_mode);
            if file_type.is_dir() {
                let child = directory.open_existing_dir(&name)?.ok_or_else(|| {
                    MaterializationError::IncompleteCache {
                        path: directory.path.join(&name),
                    }
                })?;
                collect_cache_files(&child, &logical_path, observed)?;
            } else if file_type.is_file() {
                let digest = directory.hash_relative_file(&[name.as_str()])?;
                if observed.insert(logical_path.clone(), digest).is_some() {
                    return Err(MaterializationError::InventoryMismatch {
                        reason: format!("duplicate cached path {logical_path:?}"),
                    });
                }
            } else {
                return Err(MaterializationError::InventoryMismatch {
                    reason: format!(
                        "cached entry {logical_path:?} is not a regular file or directory"
                    ),
                });
            }
        }
        Ok(())
    }

    struct SecureDirectory {
        fd: File,
        path: PathBuf,
    }

    impl SecureDirectory {
        fn open_or_create(path: &Path) -> Result<Self, MaterializationError> {
            let mut fd = if path.is_absolute() {
                File::open("/")
                    .map_err(|source| io_error("open filesystem root", Path::new("/"), source))?
            } else {
                File::open(".")
                    .map_err(|source| io_error("open current directory", Path::new("."), source))?
            };
            let mut display = if path.is_absolute() {
                PathBuf::from("/")
            } else {
                PathBuf::from(".")
            };
            for component in path.components() {
                match component {
                    Component::RootDir | Component::CurDir => continue,
                    Component::ParentDir | Component::Prefix(_) => {
                        return Err(unsafe_path(
                            &path.display().to_string(),
                            "cache root contains non-normal components",
                        ))
                    }
                    Component::Normal(name) => {
                        display.push(name);
                        fd = open_or_create_dir(&fd, name, &display)?;
                    }
                }
            }
            Ok(Self { fd, path: display })
        }

        fn open_existing_dir(&self, name: &str) -> Result<Option<Self>, MaterializationError> {
            match rfs::openat(&self.fd, name, dir_flags(), Mode::empty()) {
                Ok(fd) => Ok(Some(Self {
                    fd: fd.into(),
                    path: self.path.join(name),
                })),
                Err(Errno::NOENT) => Ok(None),
                Err(Errno::LOOP) => Err(MaterializationError::SymlinkTraversal {
                    path: self.path.join(name),
                }),
                Err(source) => Err(io_error(
                    "open cache directory without following links",
                    &self.path.join(name),
                    source,
                )),
            }
        }

        fn create_new_dir(&self, name: &str) -> Result<Self, MaterializationError> {
            rfs::mkdirat(&self.fd, name, Mode::from_raw_mode(0o700)).map_err(|source| {
                io_error(
                    "create directory without following links",
                    &self.path.join(name),
                    source,
                )
            })?;
            self.open_existing_dir(name)?
                .ok_or_else(|| MaterializationError::Io {
                    operation: "open newly created directory",
                    path: self.path.join(name),
                    source: io::Error::other("directory disappeared"),
                })
        }

        fn create_relative_dir(&self, components: &[&str]) -> Result<(), MaterializationError> {
            let mut dir = self.clone_open()?;
            for component in components {
                dir = match dir.open_existing_dir(component)? {
                    Some(existing) => existing,
                    None => dir.create_new_dir(component)?,
                };
            }
            Ok(())
        }

        fn write_relative_file(
            &self,
            components: &[&str],
            input: &mut dyn Read,
        ) -> Result<String, MaterializationError> {
            let (name, parents) = components
                .split_last()
                .ok_or_else(|| unsafe_path("", "file path has no components"))?;
            let mut dir = self.clone_open()?;
            for component in parents {
                dir = match dir.open_existing_dir(component)? {
                    Some(existing) => existing,
                    None => dir.create_new_dir(component)?,
                };
            }
            let path = dir.path.join(name);
            let fd = rfs::openat(&dir.fd, *name, file_flags(), Mode::from_raw_mode(0o600))
                .map_err(|source| {
                    io_error("create archive file without following links", &path, source)
                })?;
            let mut output: File = fd.into();
            let mut digest = Sha256::new();
            let mut buffer = [0_u8; 8192];
            loop {
                let count = input
                    .read(&mut buffer)
                    .map_err(|source| io_error("read archive entry", &path, source))?;
                if count == 0 {
                    break;
                }
                output
                    .write_all(&buffer[..count])
                    .map_err(|source| io_error("write archive file", &path, source))?;
                digest.update(&buffer[..count]);
            }
            output
                .flush()
                .map_err(|source| io_error("flush archive file", &path, source))?;
            Ok(format!("{:x}", digest.finalize()))
        }

        fn hash_relative_file(&self, components: &[&str]) -> Result<String, MaterializationError> {
            let mut file = self.open_relative_file(components)?;
            let mut digest = Sha256::new();
            let mut buffer = [0_u8; 8192];
            loop {
                let count = file
                    .read(&mut buffer)
                    .map_err(|source| io_error("read cached archive file", &self.path, source))?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
            Ok(format!("{:x}", digest.finalize()))
        }

        fn read_relative_file(&self, components: &[&str]) -> Result<Vec<u8>, MaterializationError> {
            let mut file = self.open_relative_file(components)?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|source| io_error("read completion inventory", &self.path, source))?;
            Ok(contents)
        }

        fn open_relative_file(&self, components: &[&str]) -> Result<File, MaterializationError> {
            let (name, parents) = components
                .split_last()
                .ok_or_else(|| unsafe_path("", "file path has no components"))?;
            let mut dir = self.clone_open()?;
            for component in parents {
                dir = dir.open_existing_dir(component)?.ok_or_else(|| {
                    MaterializationError::IncompleteCache {
                        path: dir.path.join(component),
                    }
                })?;
            }
            let path = dir.path.join(name);
            let fd = rfs::openat(
                &dir.fd,
                *name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| match source {
                Errno::LOOP => MaterializationError::SymlinkTraversal { path: path.clone() },
                _ => io_error("open cache file without following links", &path, source),
            })?;
            let stat = rfs::fstat(&fd)
                .map_err(|source| io_error("inspect opened cache file", &path, source))?;
            if !FileType::from_raw_mode(stat.st_mode).is_file() {
                return Err(MaterializationError::InventoryMismatch {
                    reason: format!("cached entry {:?} is not a regular file", path),
                });
            }
            Ok(fd.into())
        }

        fn promote_noreplace(
            &self,
            staging: &str,
            final_name: &str,
        ) -> Result<(), MaterializationError> {
            rfs::renameat_with(
                &self.fd,
                staging,
                &self.fd,
                final_name,
                RenameFlags::NOREPLACE,
            )
            .map_err(|source| {
                io_error(
                    "atomically promote corpus cache without replacement",
                    &self.path.join(final_name),
                    source,
                )
            })
        }

        fn clone_open(&self) -> Result<Self, MaterializationError> {
            let fd = rfs::openat(&self.fd, ".", dir_flags(), Mode::empty()).map_err(|source| {
                io_error("duplicate secure directory descriptor", &self.path, source)
            })?;
            Ok(Self {
                fd: fd.into(),
                path: self.path.clone(),
            })
        }
    }

    fn open_or_create_dir(
        parent: &File,
        name: &OsStr,
        display: &Path,
    ) -> Result<File, MaterializationError> {
        match rfs::openat(parent, name, dir_flags(), Mode::empty()) {
            Ok(fd) => Ok(fd.into()),
            Err(Errno::NOENT) => {
                rfs::mkdirat(parent, name, Mode::from_raw_mode(0o700)).map_err(|source| {
                    io_error("create cache root without following links", display, source)
                })?;
                rfs::openat(parent, name, dir_flags(), Mode::empty())
                    .map(Into::into)
                    .map_err(|source| match source {
                        Errno::LOOP => MaterializationError::SymlinkTraversal {
                            path: display.to_owned(),
                        },
                        _ => io_error("open cache root without following links", display, source),
                    })
            }
            Err(Errno::LOOP) => Err(MaterializationError::SymlinkTraversal {
                path: display.to_owned(),
            }),
            Err(source) => Err(io_error(
                "open cache root without following links",
                display,
                source,
            )),
        }
    }

    struct FreshStaging {
        dir: SecureDirectory,
        name: String,
        promoted: bool,
    }

    impl FreshStaging {
        fn create(
            root: &SecureDirectory,
            key: &CorpusCacheKey,
        ) -> Result<Self, MaterializationError> {
            for _ in 0..64 {
                let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|source| {
                        io_error("read system time", &root.path, io::Error::other(source))
                    })?
                    .as_nanos();
                let name = format!(
                    ".{}--staging-{}-{nanos}-{sequence}",
                    key.directory_name(),
                    std::process::id()
                );
                match root.create_new_dir(&name) {
                    Ok(dir) => {
                        return Ok(Self {
                            dir,
                            name,
                            promoted: false,
                        })
                    }
                    Err(MaterializationError::Io { source, .. })
                        if source.kind() == io::ErrorKind::AlreadyExists =>
                    {
                        continue
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(MaterializationError::CacheBusy {
                path: root
                    .path
                    .join(format!(".{}--staging", key.directory_name())),
            })
        }

        fn cleanup(&mut self, failures: &mut Vec<CleanupFailure>) {
            if !self.promoted {
                if let Err(source) = fs::remove_dir_all(&self.dir.path) {
                    failures.push(CleanupFailure {
                        operation: "remove staging directory",
                        path: self.dir.path.clone(),
                        source,
                    });
                }
            }
        }
    }

    struct CacheLock<'a> {
        root: &'a SecureDirectory,
        name: String,
        released: bool,
    }

    impl<'a> CacheLock<'a> {
        fn acquire(
            root: &'a SecureDirectory,
            key: &CorpusCacheKey,
        ) -> Result<Self, MaterializationError> {
            let name = format!(".{}--lock", key.directory_name());
            let path = root.path.join(&name);
            match rfs::openat(&root.fd, &name, file_flags(), Mode::from_raw_mode(0o600)) {
                Ok(fd) => {
                    drop(File::from(fd));
                    Ok(Self {
                        root,
                        name,
                        released: false,
                    })
                }
                Err(Errno::EXIST) => Err(MaterializationError::CacheBusy { path }),
                Err(Errno::LOOP) => Err(MaterializationError::SymlinkTraversal { path }),
                Err(source) => Err(io_error(
                    "acquire corpus cache lock without following links",
                    &path,
                    source,
                )),
            }
        }

        fn release(&mut self) -> Result<(), CleanupFailure> {
            if self.released {
                return Ok(());
            }
            rfs::unlinkat(&self.root.fd, &self.name, AtFlags::empty()).map_err(|source| {
                CleanupFailure {
                    operation: "release corpus cache lock",
                    path: self.root.path.join(&self.name),
                    source: source.into(),
                }
            })?;
            self.released = true;
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
use linux::materialize_linux;
