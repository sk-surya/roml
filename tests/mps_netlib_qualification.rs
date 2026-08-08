//! Small deterministic smoke slice over the pinned converted-Netlib corpus.

#[path = "support/corpus.rs"]
#[allow(dead_code)]
mod corpus;

use std::path::Path;

use roml::io::mps::MpsReader;

#[test]
fn pinned_netlib_historical_fixed_files_import_when_initialized() {
    let Some([_, netlib]) = corpus::validate_optional_corpora(Path::new("."))
        .expect("initialized optional corpora must satisfy their exact pins")
    else {
        return;
    };

    for name in ["blend.mps", "gfrd-pnc.mps", "fit2d.mps"] {
        let path = netlib.join("mps_files").join(name);
        let import = MpsReader::new()
            .read_path(&path)
            .unwrap_or_else(|error| panic!("pinned Netlib file {name} must import: {error}"));
        assert!(import.model.active_objective().is_some(), "{name}");
    }
}
