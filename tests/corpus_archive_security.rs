//! Tier-0 security tests for the optional Chinneck corpus materializer.
//!
//! These tests exercise a deterministic archive-entry seam.  They do not
//! require an initialized external corpus or an archive program/library.

#[path = "support/corpus.rs"]
mod corpus;

use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use corpus::{
    materialize_chinneck_archive, validate_optional_corpora, validate_pin, validate_pin_checkout,
    ArchiveEntry, ArchiveEntryKind, CorpusCacheKey, CorpusClassification, CorpusManifestEntry,
    ExpectedArchiveInventory, MaterializationError, PinValidationError, CORPUS_PINS,
};
use sha2::{Digest, Sha256};

static SANDBOX_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Sandbox {
    path: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let sequence = SANDBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "roml-p35-corpus-security-{}-{nanos}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("sandbox directory must be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn cache_key() -> CorpusCacheKey {
    CorpusCacheKey::new(
        "97a936498e5240d44adaf7dcfe84877fa34ce301",
        "INFfromNetlibLPs.7z",
        "a3b8e4c1641ec6f82564d78cf8402213bc410ed76ef061785e16cd761bb6c046",
    )
    .expect("test cache key must be valid")
}

fn cache_path(root: &Path) -> PathBuf {
    root.join(cache_key().directory_name())
}

fn cache_key_with_archive_identity(
    archive_identity: &str,
) -> Result<CorpusCacheKey, MaterializationError> {
    CorpusCacheKey::new(
        "97a936498e5240d44adaf7dcfe84877fa34ce301",
        archive_identity,
        "a3b8e4c1641ec6f82564d78cf8402213bc410ed76ef061785e16cd761bb6c046",
    )
}

fn file(path: &str) -> ArchiveEntry {
    ArchiveEntry::regular_file(path, b"NAME TEST\nENDATA\n".to_vec())
}

fn inventory(files: &[(&str, &[u8])]) -> ExpectedArchiveInventory {
    ExpectedArchiveInventory::new(files.iter().map(|(path, bytes)| {
        let digest = Sha256::digest(bytes);
        ((*path).to_owned(), format!("{digest:x}"))
    }))
    .expect("test inventory must be valid")
}

#[test]
fn corpus_manifest_uses_the_approved_exact_pins_and_deterministic_dispositions() {
    assert_eq!(CORPUS_PINS[0].corpus_id, "chinneck-infeasible-lps");
    assert_eq!(
        CORPUS_PINS[0].repository,
        "https://github.com/sk-surya/infeasiblelps"
    );
    assert_eq!(
        CORPUS_PINS[0].commit,
        "97a936498e5240d44adaf7dcfe84877fa34ce301"
    );
    assert_eq!(CORPUS_PINS[1].corpus_id, "netlib-lp-data");
    assert_eq!(
        CORPUS_PINS[1].repository,
        "https://github.com/sk-surya/lp-data-netlib"
    );
    assert_eq!(
        CORPUS_PINS[1].commit,
        "56257eea85b433ce6aa67d26156b36385318fd6f"
    );

    let manifest_entry = CorpusManifestEntry {
        corpus_id: CORPUS_PINS[0].corpus_id,
        source_archive_or_directory: "INFfromNetlibLPs.7z".to_owned(),
        relative_model_path: "case.mps".to_owned(),
        classification: CorpusClassification::Skipped {
            reason: "Tier 0 does not require optional corpora".to_owned(),
        },
    };
    assert!(matches!(
        manifest_entry.classification,
        CorpusClassification::Skipped { .. }
    ));
    let all_dispositions = [
        CorpusClassification::SupportedPass,
        CorpusClassification::IntentionallyUnsupported {
            reason: "unsupported semantic section".to_owned(),
        },
        CorpusClassification::Skipped {
            reason: "optional corpus unavailable".to_owned(),
        },
        CorpusClassification::Failed {
            reason: "recorded differential mismatch".to_owned(),
        },
    ];
    assert_eq!(all_dispositions.len(), 4);

    validate_pin(
        CORPUS_PINS[0],
        CORPUS_PINS[0].repository,
        CORPUS_PINS[0].commit,
    )
    .expect("the recorded pin must validate exactly");
    let error = validate_pin(CORPUS_PINS[0], CORPUS_PINS[0].repository, "deadbeef")
        .expect_err("a drifted corpus commit must be rejected");
    assert!(matches!(error, PinValidationError::CommitMismatch { .. }));

    let error = validate_pin_checkout(
        CORPUS_PINS[0],
        Path::new("testdata/corpora/infeasible-lps"),
        CORPUS_PINS[0].repository,
        CORPUS_PINS[0].commit,
        "?? local-probe.mps\n",
    )
    .expect_err("a nested checkout with untracked state must be rejected");
    assert!(matches!(error, PinValidationError::DirtyCheckout { .. }));
}

#[test]
fn absent_optional_corpora_do_not_fail_an_ordinary_test_run() {
    let sandbox = Sandbox::new();

    assert_eq!(
        validate_optional_corpora(sandbox.path())
            .expect("missing optional corpora must be allowed"),
        None
    );
}

#[test]
fn uninitialized_gitlink_directories_are_treated_as_absent() {
    let sequence = SANDBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_nanos();
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "target/roml-p35-uninitialized-gitlinks-{}-{nanos}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(repository_root.join("testdata/corpora/infeasible-lps"))
        .expect("uninitialized Chinneck gitlink directory must be creatable");
    fs::create_dir_all(repository_root.join("testdata/corpora/netlib"))
        .expect("uninitialized Netlib gitlink directory must be creatable");

    let result = validate_optional_corpora(&repository_root);

    fs::remove_dir_all(&repository_root).expect("temporary gitlink directories must be removed");
    assert_eq!(
        result.expect("uninitialized gitlinks must be ignored"),
        None
    );
}

#[test]
fn a01_rejects_posix_absolute_paths_before_writing() {
    let sandbox = Sandbox::new();

    let error = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [file("/tmp/evil.mps")],
        &ExpectedArchiveInventory::empty(),
    )
    .expect_err("absolute entry paths must be rejected");

    assert!(matches!(error, MaterializationError::UnsafePath { .. }));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a02_rejects_drive_qualified_and_unc_paths_before_writing() {
    for path in ["C:\\evil.mps", "C:/evil.mps", "\\\\server\\share\\evil.mps"] {
        let sandbox = Sandbox::new();
        let error = materialize_chinneck_archive(
            sandbox.path(),
            &cache_key(),
            [file(path)],
            &ExpectedArchiveInventory::empty(),
        )
        .expect_err("drive-qualified and UNC entry paths must be rejected");

        assert!(matches!(error, MaterializationError::UnsafePath { .. }));
        assert!(!cache_path(sandbox.path()).exists());
    }
}

#[test]
fn a03_rejects_direct_lexical_traversal_before_writing() {
    let sandbox = Sandbox::new();

    let error = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [file("../evil.mps")],
        &ExpectedArchiveInventory::empty(),
    )
    .expect_err("lexical traversal must be rejected");

    assert!(matches!(error, MaterializationError::UnsafePath { .. }));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a04_rejects_nested_lexical_traversal_before_writing() {
    let sandbox = Sandbox::new();

    let error = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [file("a/../../evil.mps")],
        &ExpectedArchiveInventory::empty(),
    )
    .expect_err("normalized traversal must be rejected");

    assert!(matches!(error, MaterializationError::UnsafePath { .. }));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a05_rejects_symlink_entries() {
    let sandbox = Sandbox::new();
    let entry = ArchiveEntry::new("model.mps", ArchiveEntryKind::Symlink, Vec::new());

    let error = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [entry],
        &ExpectedArchiveInventory::empty(),
    )
    .expect_err("symlink entries must be rejected");

    assert!(matches!(
        error,
        MaterializationError::UnsafeEntryKind { .. }
    ));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a06_rejects_hardlink_entries() {
    let sandbox = Sandbox::new();
    let entry = ArchiveEntry::new("model.mps", ArchiveEntryKind::Hardlink, Vec::new());

    let error = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [entry],
        &ExpectedArchiveInventory::empty(),
    )
    .expect_err("hardlink entries must be rejected");

    assert!(matches!(
        error,
        MaterializationError::UnsafeEntryKind { .. }
    ));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a07_rejects_device_fifo_socket_and_other_special_entries() {
    for kind in [
        ArchiveEntryKind::BlockDevice,
        ArchiveEntryKind::CharacterDevice,
        ArchiveEntryKind::Fifo,
        ArchiveEntryKind::Socket,
        ArchiveEntryKind::Special,
    ] {
        let sandbox = Sandbox::new();
        let entry = ArchiveEntry::new("model.mps", kind, Vec::new());
        let error = materialize_chinneck_archive(
            sandbox.path(),
            &cache_key(),
            [entry],
            &ExpectedArchiveInventory::empty(),
        )
        .expect_err("special entries must be rejected");

        assert!(matches!(
            error,
            MaterializationError::UnsafeEntryKind { .. }
        ));
        assert!(!cache_path(sandbox.path()).exists());
    }
}

#[test]
fn a08_writes_a_regular_nested_file_only_beneath_the_fresh_root() {
    let sandbox = Sandbox::new();

    let output = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [file("a/b/model.mps")],
        &inventory(&[("a/b/model.mps", b"NAME TEST\nENDATA\n")]),
    )
    .expect("a safe nested file must materialize");

    assert_eq!(output, cache_path(sandbox.path()));
    assert_eq!(
        fs::read(output.join("a/b/model.mps")).expect("materialized file must exist"),
        b"NAME TEST\nENDATA\n"
    );
    assert!(output.join(".roml-corpus-complete").is_file());
}

#[cfg(unix)]
#[test]
fn a09_rejects_a_filesystem_symlink_in_the_extraction_root_path() {
    use std::os::unix::fs::symlink;

    let sandbox = Sandbox::new();
    let external = sandbox.path().join("external");
    let cache_root = sandbox.path().join("cache-root");
    fs::create_dir(&external).expect("external directory must be created");
    symlink(&external, &cache_root).expect("test symlink must be created");

    let error = materialize_chinneck_archive(
        &cache_root,
        &cache_key(),
        [file("model.mps")],
        &ExpectedArchiveInventory::empty(),
    )
    .expect_err("the materializer must not traverse a filesystem symlink");

    assert!(matches!(
        error,
        MaterializationError::SymlinkTraversal { .. } | MaterializationError::Io { .. }
    ));
    assert!(!external.join(cache_key().directory_name()).exists());
}

#[test]
fn a10_payload_failure_never_promotes_or_reuses_partial_output() {
    let sandbox = Sandbox::new();
    let entries = [
        file("first.mps"),
        ArchiveEntry::failing_file(
            "second.mps",
            io::ErrorKind::Other,
            "injected archive read failure",
        ),
    ];

    let expected = inventory(&[("first.mps", b"NAME TEST\nENDATA\n"), ("second.mps", b"")]);
    let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), entries, &expected)
        .expect_err("a mid-extraction payload failure must abort materialization");

    assert!(matches!(error, MaterializationError::Io { .. }));
    assert!(!cache_path(sandbox.path()).exists());
    assert!(
        fs::read_dir(sandbox.path())
            .expect("sandbox must remain readable")
            .next()
            .is_none(),
        "partial staging output must be removed rather than reused"
    );
}

#[test]
fn a11_promotes_a_fully_validated_archive_to_a_complete_cache_key() {
    let sandbox = Sandbox::new();
    let entries = [ArchiveEntry::directory("models"), file("models/case.mps")];
    let expected = inventory(&[("models/case.mps", b"NAME TEST\nENDATA\n")]);

    let first = materialize_chinneck_archive(sandbox.path(), &cache_key(), entries, &expected)
        .expect("fully valid entries must be promoted");
    let second =
        materialize_chinneck_archive(sandbox.path(), &cache_key(), std::iter::empty(), &expected)
            .expect("a completed cache key must be reusable");

    assert_eq!(first, cache_path(sandbox.path()));
    assert_eq!(second, first);
    assert!(first.join(".roml-corpus-complete").is_file());
    assert_eq!(
        fs::read(first.join("models/case.mps")).expect("promoted file must remain available"),
        b"NAME TEST\nENDATA\n"
    );
}

#[test]
fn a12_rejects_windows_device_names_and_trailing_components_before_writing() {
    for path in [
        "CON",
        "con.txt",
        "a/PRN ",
        "a/AUX.",
        "a/COM1.mps",
        "a/LPT9 ",
        "a/ordinary. ",
    ] {
        let sandbox = Sandbox::new();
        let error = materialize_chinneck_archive(
            sandbox.path(),
            &cache_key(),
            [file(path)],
            &ExpectedArchiveInventory::empty(),
        )
        .expect_err("Windows-ambiguous archive components must be rejected");

        assert!(matches!(error, MaterializationError::UnsafePath { .. }));
        assert!(!cache_path(sandbox.path()).exists());
    }
}

#[test]
fn cache_identity_is_exactly_one_portable_safe_component() {
    for archive_identity in [
        "",
        ".",
        "..",
        "nested/archive.7z",
        "nested\\archive.7z",
        "archive:name.7z",
        "archive?.7z",
        "archive*.7z",
        "archive|name.7z",
        "archive\"name.7z",
        "archive<name.7z",
        "archive>name.7z",
        "archive\u{0001}.7z",
        "archive\u{007f}.7z",
        "COM¹",
        "LPT².txt",
        "com³.7z",
    ] {
        let error = cache_key_with_archive_identity(archive_identity)
            .expect_err("archive identity must be a portable cache-name component");
        assert!(matches!(
            error,
            MaterializationError::InvalidCacheKey { .. }
        ));
    }

    cache_key_with_archive_identity("INFfromNetlibLPs.7z")
        .expect("the approved archive identity must remain valid");
}

#[test]
fn a13_requires_expected_inventory_and_digest_before_promotion() {
    let sandbox = Sandbox::new();
    let expected = inventory(&[("models/case.mps", b"expected contents")]);

    let error = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [file("models/case.mps")],
        &expected,
    )
    .expect_err("a content digest mismatch must prevent cache promotion");

    assert!(matches!(
        error,
        MaterializationError::InventoryMismatch { .. }
    ));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a14_revalidates_expected_inventory_and_digest_before_cache_reuse() {
    let sandbox = Sandbox::new();
    let contents = b"NAME TEST\nENDATA\n";
    let expected = inventory(&[("models/case.mps", contents)]);
    let first = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [ArchiveEntry::directory("models"), file("models/case.mps")],
        &expected,
    )
    .expect("valid inventory must promote");
    fs::write(first.join("models/case.mps"), b"tampered").expect("test cache file must be mutable");

    let error =
        materialize_chinneck_archive(sandbox.path(), &cache_key(), std::iter::empty(), &expected)
            .expect_err("a tampered cache must not be reused");

    assert!(matches!(
        error,
        MaterializationError::InventoryMismatch { .. }
    ));
}

#[test]
fn a14b_rejects_an_unexpected_file_when_reusing_a_cache() {
    let sandbox = Sandbox::new();
    let expected = inventory(&[("models/case.mps", b"NAME TEST\nENDATA\n")]);
    let cache = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [ArchiveEntry::directory("models"), file("models/case.mps")],
        &expected,
    )
    .expect("valid inventory must promote");
    fs::write(cache.join("unexpected.mps"), b"unreviewed cache content")
        .expect("test cache must accept an injected file");

    let error =
        materialize_chinneck_archive(sandbox.path(), &cache_key(), std::iter::empty(), &expected)
            .expect_err("an unexpected cache file invalidates exact inventory reuse");

    assert!(matches!(
        error,
        MaterializationError::InventoryMismatch { .. }
    ));
}

#[cfg(target_os = "linux")]
fn replace_with_fifo(path: &Path) {
    use rustix::fs::{self as rfs, Mode};
    use std::fs::File;

    let parent = path.parent().expect("cache entry must have a parent");
    let name = path.file_name().expect("cache entry must have a name");
    fs::remove_file(path).expect("cached regular file must be removable for adversarial test");
    let parent_fd = File::open(parent).expect("cache parent must be openable");
    rfs::mkfifoat(&parent_fd, name, Mode::from_raw_mode(0o600))
        .expect("adversarial FIFO must be creatable");
}

#[cfg(target_os = "linux")]
#[test]
fn a15_rejects_a_fifo_completion_marker_without_blocking_on_cache_reuse() {
    let sandbox = Sandbox::new();
    let expected = inventory(&[("models/case.mps", b"NAME TEST\nENDATA\n")]);
    let cache = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [ArchiveEntry::directory("models"), file("models/case.mps")],
        &expected,
    )
    .expect("valid inventory must promote");
    replace_with_fifo(&cache.join(".roml-corpus-complete"));

    let error =
        materialize_chinneck_archive(sandbox.path(), &cache_key(), std::iter::empty(), &expected)
            .expect_err("a FIFO completion marker must not be reused");

    assert!(matches!(
        error,
        MaterializationError::IncompleteCache { .. }
            | MaterializationError::InventoryMismatch { .. }
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn a16_rejects_a_fifo_data_file_without_blocking_on_cache_reuse() {
    let sandbox = Sandbox::new();
    let expected = inventory(&[("models/case.mps", b"NAME TEST\nENDATA\n")]);
    let cache = materialize_chinneck_archive(
        sandbox.path(),
        &cache_key(),
        [ArchiveEntry::directory("models"), file("models/case.mps")],
        &expected,
    )
    .expect("valid inventory must promote");
    replace_with_fifo(&cache.join("models/case.mps"));

    let error =
        materialize_chinneck_archive(sandbox.path(), &cache_key(), std::iter::empty(), &expected)
            .expect_err("a FIFO cache file must not be hashed or reused");

    assert!(matches!(
        error,
        MaterializationError::InventoryMismatch { .. }
    ));
}

#[cfg(unix)]
#[test]
fn a17_preserves_the_payload_error_when_staging_cleanup_and_lock_release_fail() {
    use std::os::unix::fs::PermissionsExt;

    struct PermissionRevokingReader {
        cache_root: PathBuf,
    }

    impl io::Read for PermissionRevokingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            fs::set_permissions(&self.cache_root, fs::Permissions::from_mode(0o500))?;
            Err(io::Error::other(
                "injected payload failure after permission revocation",
            ))
        }
    }

    let sandbox = Sandbox::new();
    let expected = inventory(&[("case.mps", b"")]);
    let entry = ArchiveEntry::with_reader(
        "case.mps",
        ArchiveEntryKind::RegularFile,
        PermissionRevokingReader {
            cache_root: sandbox.path().to_owned(),
        },
    );

    let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), [entry], &expected)
        .expect_err("permission-revoked cleanup must fail visibly");

    fs::set_permissions(sandbox.path(), fs::Permissions::from_mode(0o700))
        .expect("test must restore sandbox permissions");
    assert!(matches!(error, MaterializationError::WithCleanup { .. }));
    assert!(
        error.source().is_some(),
        "the original payload error must remain the error source"
    );
}
