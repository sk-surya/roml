//! Tier-0 security tests for the optional Chinneck corpus materializer.
//!
//! These tests exercise a deterministic archive-entry seam.  They do not
//! require an initialized external corpus or an archive program/library.

#[path = "support/corpus.rs"]
mod corpus;

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use corpus::{
    materialize_chinneck_archive, validate_optional_corpora, validate_pin, ArchiveEntry,
    ArchiveEntryKind, CorpusCacheKey, CorpusClassification, CorpusManifestEntry,
    MaterializationError, PinValidationError, CORPUS_PINS,
};

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

fn file(path: &str) -> ArchiveEntry {
    ArchiveEntry::regular_file(path, b"NAME TEST\nENDATA\n".to_vec())
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
fn a01_rejects_posix_absolute_paths_before_writing() {
    let sandbox = Sandbox::new();

    let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), [file("/tmp/evil.mps")])
        .expect_err("absolute entry paths must be rejected");

    assert!(matches!(error, MaterializationError::UnsafePath { .. }));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a02_rejects_drive_qualified_and_unc_paths_before_writing() {
    for path in ["C:\\evil.mps", "C:/evil.mps", "\\\\server\\share\\evil.mps"] {
        let sandbox = Sandbox::new();
        let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), [file(path)])
            .expect_err("drive-qualified and UNC entry paths must be rejected");

        assert!(matches!(error, MaterializationError::UnsafePath { .. }));
        assert!(!cache_path(sandbox.path()).exists());
    }
}

#[test]
fn a03_rejects_direct_lexical_traversal_before_writing() {
    let sandbox = Sandbox::new();

    let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), [file("../evil.mps")])
        .expect_err("lexical traversal must be rejected");

    assert!(matches!(error, MaterializationError::UnsafePath { .. }));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a04_rejects_nested_lexical_traversal_before_writing() {
    let sandbox = Sandbox::new();

    let error =
        materialize_chinneck_archive(sandbox.path(), &cache_key(), [file("a/../../evil.mps")])
            .expect_err("normalized traversal must be rejected");

    assert!(matches!(error, MaterializationError::UnsafePath { .. }));
    assert!(!cache_path(sandbox.path()).exists());
}

#[test]
fn a05_rejects_symlink_entries() {
    let sandbox = Sandbox::new();
    let entry = ArchiveEntry::new("model.mps", ArchiveEntryKind::Symlink, Vec::new());

    let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), [entry])
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

    let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), [entry])
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
        let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), [entry])
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

    let output =
        materialize_chinneck_archive(sandbox.path(), &cache_key(), [file("a/b/model.mps")])
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

    let error = materialize_chinneck_archive(&cache_root, &cache_key(), [file("model.mps")])
        .expect_err("the materializer must not traverse a filesystem symlink");

    assert!(matches!(
        error,
        MaterializationError::SymlinkTraversal { .. }
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

    let error = materialize_chinneck_archive(sandbox.path(), &cache_key(), entries)
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

    let first = materialize_chinneck_archive(sandbox.path(), &cache_key(), entries)
        .expect("fully valid entries must be promoted");
    let second = materialize_chinneck_archive(sandbox.path(), &cache_key(), std::iter::empty())
        .expect("a completed cache key must be reusable");

    assert_eq!(first, cache_path(sandbox.path()));
    assert_eq!(second, first);
    assert!(first.join(".roml-corpus-complete").is_file());
    assert_eq!(
        fs::read(first.join("models/case.mps")).expect("promoted file must remain available"),
        b"NAME TEST\nENDATA\n"
    );
}
