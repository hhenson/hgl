//! Mutable constructor roots whose children need the existing dynamic growth policy.
use crate::ir::{Kind, Node, Value};
use std::collections::BTreeSet;
fn root(value: &Value) -> Option<usize> {
    if let Kind::MutableLocal(id) = value.kind {
        Some(id)
    } else if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        root(parent)
    } else {
        None
    }
}
pub(crate) fn locals(node: &Node) -> BTreeSet<usize> {
    let mut growing = BTreeSet::new();
    crate::ir_visit::hooks(node, &mut |value| {
        if let Kind::Push(parent, _) = &value.kind
            && let Some(id) = root(parent)
        {
            growing.insert(id);
        }
        !matches!(value.kind, Kind::ValueCall(..))
    });
    growing
}
