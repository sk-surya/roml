//! Ordered storage and deterministic selection for named MPS rim vectors.

use super::{MpsDiagnostic, MpsError, MpsErrorKind, MpsVectorSelection};

/// One named MPS rim vector, retaining its entries in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MpsNamedVector<T> {
    name: String,
    entries: Vec<T>,
}

impl<T> MpsNamedVector<T> {
    /// Returns the exact vector name.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns entries in their encounter order.
    pub(crate) fn entries(&self) -> &[T] {
        &self.entries
    }
}

/// Named MPS vectors in first-seen order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MpsNamedVectors<T> {
    vectors: Vec<MpsNamedVector<T>>,
}

impl<T> MpsNamedVectors<T> {
    /// Creates empty vector storage.
    pub(crate) fn new() -> Self {
        Self {
            vectors: Vec::new(),
        }
    }

    /// Appends an entry, preserving vector and entry encounter order.
    pub(crate) fn push(&mut self, name: impl Into<String>, entry: T) {
        let name = name.into();
        if let Some(vector) = self.vectors.iter_mut().find(|vector| vector.name == name) {
            vector.entries.push(entry);
        } else {
            self.vectors.push(MpsNamedVector {
                name,
                entries: vec![entry],
            });
        }
    }

    /// Returns vectors in first-seen order.
    pub(crate) fn vectors(&self) -> &[MpsNamedVector<T>] {
        &self.vectors
    }

    /// Selects one vector according to the P35 deterministic policy.
    pub(crate) fn select(
        &self,
        selection: &MpsVectorSelection,
    ) -> Result<Option<&MpsNamedVector<T>>, MpsError> {
        match selection {
            MpsVectorSelection::First => Ok(self.vectors.first()),
            MpsVectorSelection::Named(name) => self
                .vectors
                .iter()
                .find(|vector| vector.name == *name)
                .map(Some)
                .ok_or_else(|| {
                    MpsError::new(
                        MpsErrorKind::UnknownVector,
                        MpsDiagnostic::new()
                            .with_entity(name)
                            .with_message("the requested MPS vector was not staged"),
                    )
                }),
            MpsVectorSelection::None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MpsNamedVectors;
    use crate::io::mps::{MpsErrorKind, MpsVectorSelection};

    #[test]
    fn selects_named_vectors_deterministically_or_disables_them() {
        let mut vectors = MpsNamedVectors::new();
        vectors.push("baseline", 10_u8);
        vectors.push("stress", 20_u8);
        vectors.push("baseline", 11_u8);

        let first = vectors.select(&MpsVectorSelection::First).unwrap().unwrap();
        assert_eq!(first.name(), "baseline");
        assert_eq!(first.entries(), &[10, 11]);

        let named = vectors
            .select(&MpsVectorSelection::Named("stress".to_owned()))
            .unwrap()
            .unwrap();
        assert_eq!(named.name(), "stress");
        assert_eq!(named.entries(), &[20]);

        assert_eq!(vectors.select(&MpsVectorSelection::None).unwrap(), None);
        assert!(matches!(
            vectors.select(&MpsVectorSelection::Named("missing".to_owned())),
            Err(error) if error.kind() == &MpsErrorKind::UnknownVector
        ));
    }
}
