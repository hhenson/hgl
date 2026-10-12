//! Cold independently owned native data retains its explicitly supplied provider operations.
use hgl_types::NodeResult;
use std::cmp::Ordering;
/// Statically linked provider operations; source checking never constructs this table.
#[derive(Debug)]
pub struct Operations {
    /// Owning copy after the typed provider has validated the source encoding.
    pub copy: fn(&[u8], &mut Vec<u8>),
    /// Provider equality, independent of physical encoding equality.
    pub equal: fn(&[u8], &[u8]) -> NodeResult<bool>,
    /// Provider hash consistent with equality.
    pub hash: fn(&[u8]) -> NodeResult<u64>,
    /// Provider partial comparison; callers enforce checked capability declarations.
    pub compare: fn(&[u8], &[u8]) -> NodeResult<Option<Ordering>>,
}
/// An owning cold native value; its enclosing checked type retains canonical identity.
#[derive(Debug)]
pub struct NativeValue {
    data: Vec<u8>,
    operations: &'static Operations,
}
impl NativeValue {
    /// Accept independently owned data from an already validated typed provider boundary.
    pub fn new(data: Vec<u8>, operations: &'static Operations) -> Self {
        Self { data, operations }
    }
    /// Borrow the opaque encoding only at an explicitly selected provider boundary.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    /// Read the statically retained provider callbacks.
    pub fn operations(&self) -> &'static Operations {
        self.operations
    }
}
impl Clone for NativeValue {
    fn clone(&self) -> Self {
        let mut data = Vec::with_capacity(self.data.len());
        (self.operations.copy)(&self.data, &mut data);
        Self::new(data, self.operations)
    }
}
