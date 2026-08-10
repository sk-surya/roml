//! Contract coverage for the exact P36 converted-Netlib inventory.

#[allow(dead_code)]
#[path = "../../tests/support/corpus.rs"]
mod corpus;

use std::{collections::BTreeSet, fs, path::Path};

fn expected_netlib_names() -> BTreeSet<String> {
    let manifest = include_str!("../../.planning/phases/36-mps-writeback/36-NETLIB-MANIFEST.md");
    let mut in_names = false;
    let mut names = BTreeSet::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line == "```text" {
            in_names = true;
        } else if in_names && line == "```" {
            break;
        } else if in_names && !line.is_empty() {
            names.insert(line.to_owned());
        }
    }
    names
}

#[test]
fn frozen_manifest_has_exactly_94_unique_names() {
    let names = expected_netlib_names();
    assert_eq!(
        names.len(),
        94,
        "the reviewed P36 manifest must remain exact"
    );
    assert!(names.iter().all(|name| name.ends_with(".mps")));
}

#[test]
fn initialized_netlib_checkout_matches_the_frozen_manifest() {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let Some([_, netlib]) = corpus::validate_optional_corpora(&repository_root)
        .expect("an initialized optional corpus must satisfy its exact pin")
    else {
        return;
    };

    let directory = netlib.join("mps_files");
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(&directory).expect("pinned Netlib mps_files directory") {
        let entry = entry.expect("read Netlib directory entry");
        let metadata = entry.metadata().expect("stat Netlib directory entry");
        if metadata.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mps"))
        {
            actual.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }

    assert_eq!(actual, expected_netlib_names());
}
