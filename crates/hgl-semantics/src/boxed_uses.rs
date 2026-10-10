//! Immutable ordinary locals consumed exclusively as complete box values.
use crate::ir::{Kind, Statement, Value};
use std::collections::BTreeSet;
fn scan(
    value: &Value,
    candidates: &BTreeSet<usize>,
    seen: &mut BTreeSet<usize>,
    other: &mut BTreeSet<usize>,
) -> bool {
    if let Kind::Unary(op, child) = &value.kind
        && op == "any"
        && let Kind::Local(id) = child.kind
        && candidates.contains(&id)
    {
        seen.insert(id);
        return false;
    }
    if let Kind::Local(id) = value.kind
        && candidates.contains(&id)
    {
        other.insert(id);
    }
    if let Kind::ValueCall(args, _) = &value.kind {
        for arg in args {
            crate::ir_visit::value(arg, &mut |v| scan(v, candidates, seen, other));
        }
        return false;
    }
    true
}
/// Select immutable lexical owners with no ordinary reads or helper capture.
pub fn exclusive(body: &[Statement], candidate: impl Fn(&Value) -> bool) -> BTreeSet<usize> {
    let mut candidates = BTreeSet::new();
    crate::ir_visit::nodes(body, &mut |s| {
        if let Statement::Let(id, v) = s
            && candidate(v)
        {
            candidates.insert(*id);
        }
    });
    let mut seen = BTreeSet::new();
    let mut other = BTreeSet::new();
    crate::ir_visit::statements(body, &mut |v| scan(v, &candidates, &mut seen, &mut other));
    seen.difference(&other).copied().collect()
}
