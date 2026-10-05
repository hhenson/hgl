//! Immutable finite declaration batches with exact nominal edge identity.
use std::cmp::Ordering;
/// One concrete declaration's ordered field schema and presence metadata.
#[derive(Debug, Clone)]
pub struct Definition<I, T> {
    identity: I,
    fields: Vec<(String, T)>,
    optional: Vec<usize>,
}
impl<I, T> Definition<I, T> {
    /// Retain a semantically checked concrete declaration.
    pub fn new(identity: I, fields: Vec<(String, T)>, optional: Vec<usize>) -> Self {
        Self {
            identity,
            fields,
            optional,
        }
    }
    /// Exact concrete specialization identity.
    pub fn identity(&self) -> &I {
        &self.identity
    }
    /// Declaration-ordered fields.
    pub fn fields(&self) -> &[(String, T)] {
        &self.fields
    }
    /// Optional declaration positions.
    pub fn optional(&self) -> &[usize] {
        &self.optional
    }
}
/// Selected nominal root and its independently owned finite definition closure.
#[derive(Debug, Clone)]
pub struct Batch<I, T> {
    identity: I,
    definitions: Vec<Definition<I, T>>,
}
impl<I: Ord, T> Batch<I, T> {
    /// Canonicalize complete definitions and reject ambiguous or missing roots.
    pub fn new(identity: I, mut definitions: Vec<Definition<I, T>>) -> Result<Self, String> {
        definitions.sort_by(|a, b| a.identity.cmp(&b.identity));
        if definitions
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
        {
            return Err("duplicate exact nominal definition".into());
        }
        if !definitions
            .iter()
            .any(|definition| definition.identity == identity)
        {
            return Err("unresolved nominal batch root".into());
        }
        Ok(Self {
            identity,
            definitions,
        })
    }
    /// Retain an edge identity; its enclosing complete batch must resolve it.
    pub fn reference(identity: I) -> Self {
        Self {
            identity,
            definitions: Vec::new(),
        }
    }
    /// Exact selected specialization identity, independent of traversal order.
    pub fn identity(&self) -> &I {
        &self.identity
    }
    /// Complete sorted definitions, or no definitions for an internal edge.
    pub fn definitions(&self) -> &[Definition<I, T>] {
        &self.definitions
    }
    /// Resolve one exact edge within this complete batch.
    pub fn definition(&self, identity: &I) -> Result<&Definition<I, T>, String> {
        self.definitions
            .iter()
            .find(|definition| &definition.identity == identity)
            .ok_or_else(|| "unresolved recursive nominal edge".into())
    }
}
impl<I: Ord, T> PartialEq for Batch<I, T> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl<I: Ord, T> Eq for Batch<I, T> {}
impl<I: Ord, T> PartialOrd for Batch<I, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<I: Ord, T> Ord for Batch<I, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.identity.cmp(&other.identity)
    }
}
