//! Structural variable-array namespace (P2A).
//!
//! A variable array owns a *reservation* (`base -> len`), not N eagerly
//! allocated element strings. A candidate name is an implicit element of
//! a reservation only when it matches exactly what
//! [`element_name`](super::arrays::element_name) would generate: the base,
//! `[`, a canonical nonnegative decimal, `]`. Anything else (`x[01]`,
//! `x[-1]`, `x[1x]`, overflowed integers) is an ordinary explicit name.
//!
//! Occupied generated-looking names that are NOT implicit elements —
//! explicit scalars, eager parameter elements, array bases of any kind —
//! live in [`ModelState::explicit_indices`](super::model::ModelState),
//! keyed by base. Constraint names are deliberately excluded: variable
//! paths never consulted `con_names`, and that asymmetry is preserved.
//!
//! All namespace answers flow through the helpers here; call sites only
//! format their own error messages.

use std::collections::BTreeSet;

use super::model::ModelState;

/// Split a candidate explicit name into `(base, flat index)` when its
/// final suffix is exactly `[` + canonical decimal + `]`.
///
/// Operates on the final suffix only, so bracketed bases work:
/// `x[0][1]` splits to (`x[0]`, 1). Returns `None` for anything
/// `element_name` could never generate (empty digits, non-digits,
/// leading zeros, `usize` overflow) — those are ordinary explicit names.
/// Never panics.
pub(crate) fn split_generated(name: &str) -> Option<(&str, usize)> {
    let bytes = name.as_bytes();
    if bytes.last() != Some(&b']') {
        return None;
    }
    let open = bytes.iter().rposition(|&b| b == b'[')?;
    let digits = &name[open + 1..name.len() - 1];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    // `parse` fails (rather than wraps) on overflow — no panic path.
    let idx: usize = digits.parse().ok()?;
    Some((&name[..open], idx))
}

impl ModelState {
    /// Exact occupancy in the shared var/param/array namespace (no
    /// constraint names, no implicit elements).
    fn name_taken(&self, name: &str) -> bool {
        self.var_names.contains_key(name)
            || self.param_names.contains_key(name)
            || self.array_names.contains(name)
    }

    /// Whether `name` is an implicit element of a variable-array
    /// reservation (`base` reserved with `len`, `idx < len`).
    pub(crate) fn implicit_var_element(&self, name: &str) -> bool {
        match split_generated(name) {
            Some((base, idx)) => self.var_array_lens.get(base).is_some_and(|len| idx < *len),
            None => false,
        }
    }

    /// Full explicit-name conflict for the var/param/array namespace:
    /// exact occupancy or an implicit variable-array element. Replaces
    /// the three-map `contains_key` chains at scalar creation sites.
    pub(crate) fn explicit_name_conflicts(&self, name: &str) -> bool {
        self.name_taken(name) || self.implicit_var_element(name)
    }

    /// Prospective variable-array check: the first occupied
    /// generated-looking element under `base` in `[0, len)$, if any.
    /// Returns it in generated spelling for error messages. Exact base
    /// checks stay at the call site, unchanged.
    pub(crate) fn prospective_array_conflicts(&self, base: &str, len: usize) -> Option<String> {
        self.explicit_indices.get(base).and_then(|set| {
            set.range(..len)
                .next()
                .map(|idx| super::arrays::element_name(base, *idx))
        })
    }

    /// Record an occupied generated-looking name: explicit scalars,
    /// eager parameter elements, and array bases of any kind. Names that
    /// cannot parse (and constraint names, by design) are ignored.
    pub(crate) fn index_explicit_name(&mut self, name: &str) {
        if let Some((base, idx)) = split_generated(name) {
            self.explicit_indices
                .entry(base.to_string())
                .or_default()
                .insert(idx);
        }
    }

    /// Canonical variable count for diagnostics. Array elements are
    /// intentionally absent from `var_names` after P2A, so cardinality
    /// there no longer answers this; the core entity store does.
    pub(crate) fn variable_count(&self) -> usize {
        self.model.num_variables()
    }

    /// Reverse index for tests/diagnostics: occupied generated-looking
    /// indices under `base` (explicit names and foreign array bases).
    #[cfg(test)]
    pub(crate) fn indexed_explicit(&self, base: &str) -> Vec<usize> {
        self.explicit_indices
            .get(base)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }
}

/// Reverse index type: base name -> occupied generated-looking indices.
pub(crate) type ExplicitIndices = std::collections::HashMap<String, BTreeSet<usize>>;

#[cfg(test)]
mod tests {
    use super::split_generated;

    #[test]
    fn canonical_spellings_split() {
        assert_eq!(split_generated("x[0]"), Some(("x", 0)));
        assert_eq!(split_generated("x[1]"), Some(("x", 1)));
        assert_eq!(split_generated("x[123]"), Some(("x", 123)));
        assert_eq!(split_generated("x[0][1]"), Some(("x[0]", 1)));
        assert_eq!(split_generated("[5]"), Some(("", 5)));
    }

    #[test]
    fn non_generated_spellings_reject() {
        for bad in [
            "x", "x[]", "x[01]", "x[00]", "x[-1]", "x[+1]", "x[1.0]", "x[1x]", "x[ 1 ]", "x[1 ]",
            "x[ 1]", "x[", "x]", "x[1", "[", "]", "",
        ] {
            assert_eq!(split_generated(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn overflow_is_ordinary_name_not_panic() {
        assert_eq!(split_generated("x[99999999999999999999999]"), None);
        // usize::MAX itself parses (it is a canonical spelling in theory).
        let max = format!("x[{}]", usize::MAX);
        assert_eq!(split_generated(&max), Some(("x", usize::MAX)));
    }

    /// Reservation + reverse-index protocol on a real `ModelState`:
    /// prospective arrays consult the index, never formatted strings.
    #[test]
    fn reservation_and_index_protocol() {
        use super::super::model::ModelState;
        use std::collections::HashMap;
        let mut state = ModelState {
            model: roml::Model::new(),
            name: String::new(),
            var_names: HashMap::new(),
            param_names: HashMap::new(),
            con_names: HashMap::new(),
            var_array_lens: HashMap::new(),
            explicit_indices: HashMap::new(),
            array_names: std::collections::HashSet::new(),
            param_array_shapes: HashMap::new(),
            param_array_ids: HashMap::new(),
            obj_coeffs: HashMap::new(),
            con_coeffs: HashMap::new(),
            has_complex_deps: false,
            has_discrete: false,
            pending: false,
            py_revision: 0,
            bound_deps: Vec::new(),
        };
        // Explicit scalar "x[5]" then prospective array "x" len 100.
        state.index_explicit_name("x[5]");
        // Ugly spellings never enter the index.
        state.index_explicit_name("x[01]");
        state.index_explicit_name("plain");
        assert_eq!(state.indexed_explicit("x"), vec![5]);
        assert_eq!(
            state.prospective_array_conflicts("x", 100),
            Some("x[5]".to_string())
        );
        // Out-of-range explicits do not collide; unknown bases are clear.
        assert_eq!(state.prospective_array_conflicts("x", 5), None);
        assert_eq!(state.prospective_array_conflicts("y", 100), None);
        // Reservations answer implicit membership without strings.
        state.var_array_lens.insert("x".to_string(), 100);
        assert!(state.implicit_var_element("x[5]"));
        assert!(!state.implicit_var_element("x[100]"));
        assert!(!state.implicit_var_element("x[01]"));
        assert!(!state.implicit_var_element("y[5]"));
        assert!(state.explicit_name_conflicts("x[5]"));
        assert!(!state.explicit_name_conflicts("x[100]"));
    }
}
