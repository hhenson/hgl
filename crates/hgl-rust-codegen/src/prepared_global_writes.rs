//! Ordered global presence follows complete prepared payload replacement.
use crate::{layouts::global_type, prepared_hooks::replace, values::condition_code as emit};
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::Ty;
pub(crate) fn set(plan: &Plan, id: usize, v: &Value) -> String {
    if crate::finite_domains::prepared(plan)
        && (matches!(v.ty, Ty::Any | Ty::Native(_)) || matches!(v.kind, Kind::Construct(_)))
    {
        let write = if matches!(v.kind, Kind::Construct(_)) {
            replace(plan, v, "destination")
        } else {
            crate::any_prepared::write(plan, v, "destination", |v| emit(plan, v))
        };
        return format!(
            "{{let destination=_ctx.global_state().destination(self.global{id});{write}_ctx.global_state().mark_present(self.global{id});}}"
        );
    }
    if v.snapshot {
        let marker = global_type(&v.ty);
        let source = crate::snapshot_slots::projection(v, |v| emit(plan, v));
        return format!(
            "{{let source={source};let globals=_ctx.global_state();let destination=globals.destination(self.global{id});<{marker} as hgl_store::PreparedValue>::check_slots(globals.values(),source,globals.values(),destination)?;<{marker} as hgl_store::PreparedValue>::copy_within(globals.values_mut(),source,destination);globals.mark_present(self.global{id});}}"
        );
    }
    if !crate::finite_domains::prepared(plan) {
        return format!(
            "{{let value={};_ctx.global_state().set(self.global{id},&value)?;}}",
            emit(plan, v)
        );
    }
    let marker = global_type(&v.ty);
    format!(
        "{{let value={};let globals=_ctx.global_state();let slot=globals.destination(self.global{id});<{marker} as hgl_store::PreparedValue>::check_native(globals.values(),slot,&value)?;<{marker} as hgl_store::PreparedValue>::copy_native(globals.values_mut(),slot,&value);globals.mark_present(self.global{id});}}",
        emit(plan, v)
    )
}
