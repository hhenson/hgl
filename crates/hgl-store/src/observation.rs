//! Immutable temporal observations separated from publication authority.
use crate::bindings::{Bindings, InputId};
use crate::columns::{Columns, Scalar};
use hgl_types::{NodeError, NodeResult};
/// Read-only temporal sources can coexist with independently writable global records.
#[derive(Debug, Clone, Copy)]
pub struct Observation<'a> {
    /// Scalar source columns selected by generated types.
    pub columns: &'a Columns,
    /// Temporal validity, deltas and prepared projections.
    pub bindings: &'a Bindings,
    /// Whole-value atomic source columns.
    pub atomic: &'a crate::atomic::Arena,
    /// Retained ordinary arrival windows.
    pub rolling: &'a crate::rolling::Arena,
    /// Exact retained scalar key identities.
    pub keys: &'a crate::keys::Keys,
}
impl Observation<'_> {
    /// Borrow a valid scalar input without constructing an owning value.
    pub fn scalar<T: Scalar>(&self, input: InputId) -> NodeResult<&T> {
        if !self.bindings.valid(input) {
            return Err(NodeError::new("prepared input is invalid"));
        }
        Ok(&T::column(self.columns)[self.bindings.input(input).slot as usize])
    }
}
