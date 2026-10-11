//! Shared immutable source projections preserve prepared owning buffers.
use crate::values::{atomic_place, borrowed_place};
use hgl_semantics::ir::{Kind, Plan, Value};
/// Select immutable typed ordinary storage without retaining an intermediate owner.
pub(crate) fn source(plan: &Plan, value: &Value) -> Option<(&'static str, String)> {
    if let Some(source) = crate::constructor_owners::source(plan, value) {
        return Some(("global_values", source));
    }
    if matches!(value.ty, hgl_source::Ty::Any | hgl_source::Ty::Native(_))
        && let Kind::GlobalGet(id) = value.kind
    {
        return Some((
            "global_values",
            format!("_ctx.global_state().borrow(self.global{id})?"),
        ));
    }
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
    if let Some(slot) = borrowed_place(plan, value) {
        return Some(("global_values", slot));
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
    None
}

/// Read a typed configuration or atomic projection without retaining its parent.
pub(crate) fn read(plan: &Plan, v: &Value, emit: impl Fn(&Value) -> String) -> Option<String> {
    if let Some(slot) = crate::constructor_owners::source(plan, v) {
        return Some(format!(
            "{{let source={slot};source.read(_ctx.store().global_values())?}}"
        ));
    }
    if let Some(slot) = crate::source_slots::projection(v, emit) {
        return Some(format!("({slot}).read(&self.configuration_columns)?"));
    }
    atomic_place(plan, v).map(|slot| format!("({slot}).read(_ctx.store().atomic_values())?"))
}
