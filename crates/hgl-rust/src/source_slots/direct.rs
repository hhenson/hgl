//! Shared immutable source projections preserve prepared owning buffers.
use crate::values::{atomic_place, borrowed_place};
use hgl_semantics::ir::{Kind, Plan, Value};
/// Select immutable typed ordinary storage without retaining an intermediate owner.
pub(crate) fn source(plan: &Plan, value: &Value) -> Option<(&'static str, String)> {
    if let Kind::Unary(op, source) = &value.kind
        && op == "atomic_value"
    {
        return self::source(plan, source);
    }
    if let Some(slot) = atomic_place(plan, value) {
        return Some(("atomic_values", slot));
    }
    if let Some(slot) =
        crate::source_slots::projection(value, |v| crate::values::condition_code(plan, v))
    {
        return Some(("configuration_columns", slot));
    }
    if value.snapshot
        && matches!(
            value.kind,
            Kind::Local(_)
                | Kind::MutableLocal(_)
                | Kind::GeneratorLocal(_)
                | Kind::Field(..)
                | Kind::Index(..)
        )
    {
        return Some((
            "global_values",
            crate::snapshot_slots::projection(value, |v| crate::values::condition_code(plan, v)),
        ));
    }
    borrowed_place(plan, value).map(|slot| ("global_values", slot))
}

/// Read a typed configuration or atomic projection without retaining its parent.
pub(crate) fn read(plan: &Plan, v: &Value, emit: impl Fn(&Value) -> String) -> Option<String> {
    if let Some(slot) = crate::source_slots::projection(v, emit) {
        return Some(format!("({slot}).read(&self.configuration_columns)?"));
    }
    atomic_place(plan, v).map(|slot| format!("({slot}).read(_ctx.store().atomic_values())?"))
}
