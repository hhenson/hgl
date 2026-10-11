//! Prepared publication and ordered global hook writes share typed source slots.
use crate::layouts::global_type;
use crate::values::condition_code as emit;
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::Ty;
pub(crate) fn publication(plan: &Plan, value: &Value, result: Option<&Ty>) -> Option<String> {
    if !crate::finite_domains::prepared(plan) || plan.recording.is_none() {
        return None;
    }
    if let Some(code) = crate::any_values::publication(value) {
        return Some(code);
    }
    if let Some(id) = crate::scalars::bytes_input(value) {
        let method = if result == Some(&Ty::Bytes) {
            "bytes_from_list"
        } else if matches!(result, Some(Ty::Rolling(payload, _)) if **payload == Ty::Bytes) {
            "rolling_bytes_from_list"
        } else {
            return None;
        };
        return Some(format!(
            "_ctx.prepared().{method}(self.input{id},self._output)?;"
        ));
    }
    if let Kind::Configuration(id) = value.kind
        && (value.ty.atomic_payload() || matches!(value.ty, Ty::Delta(_)))
    {
        return Some(crate::observed::apply(
            result?,
            "self._output",
            "&self.configuration_columns",
            &format!("self.configuration_slot{id}"),
        ));
    }
    if matches!(result, Some(Ty::Atomic(_)))
        && !value.snapshot
        && !crate::execution_proof::owning_argument(&value.ty)
    {
        return Some(format!(
            "let publication={};_ctx.prepared().atomic(self._output,&publication)?;",
            emit(plan, value)
        ));
    }
    if let Some(code) = crate::direct_deltas::publish(value, |v| emit(plan, v)) {
        return Some(code);
    }
    if let Some(code) = crate::observed::text(value, result) {
        return Some(code);
    }
    crate::observed::forwarded(value, result)
}
fn write(plan: &Plan, v: &Value, slot: &str, prelude: &mut Vec<String>) -> String {
    if (!v.snapshot || v.ty == Ty::Any)
        && let Some((arena, source)) = crate::source_slots::source(plan, v)
    {
        let marker = global_type(&v.ty);
        let id = prelude.len();
        prelude.push(format!("let prepared_source{id}={source};"));
        let source = crate::source_slots::columns(arena);
        return if arena == "global_values" {
            format!(
                "<{marker} as hgl_store::PreparedValue>::check_slots(columns,prepared_source{id},columns,{slot})?;<{marker} as hgl_store::PreparedValue>::copy_within(columns,prepared_source{id},{slot});"
            )
        } else {
            format!(
                "<{marker} as hgl_store::PreparedValue>::check_slots({source},prepared_source{id},columns,{slot})?;<{marker} as hgl_store::PreparedValue>::copy_between({source},prepared_source{id},columns,{slot});"
            )
        };
    }
    if let Some(code) =
        crate::constructor_fields::fields(v, slot, |value, slot| write(plan, value, slot, prelude))
    {
        return code;
    }
    if let Kind::Unary(op, argument) = &v.kind
        && op == "any"
    {
        if argument.ty == Ty::Any {
            return write(plan, argument, slot, prelude);
        }
        let id = prelude.len();
        if let Some((arena, source)) = crate::source_slots::source(plan, argument) {
            prelude.push(format!("let prepared_source{id}={source};"));
            let columns = crate::source_slots::columns(arena);
            return crate::any_values::prepared_source(
                &argument.ty,
                columns,
                &format!("prepared_source{id}"),
                slot,
            );
        }
        if let Some(code) =
            crate::any_expression::encoding(plan, argument, &|v| emit(plan, v), prelude)
        {
            return crate::any_expression::compose(&code, slot);
        }
        prelude.push(format!("let prepared_item{id}={};", emit(plan, argument)));
        return crate::any_values::prepared_native(
            &argument.ty,
            &format!("prepared_item{id}"),
            slot,
        );
    }
    if let Kind::Query(op, args) = &v.kind
        && op == "delta_value"
    {
        let arg = &args[0];
        let Kind::Input(id, _) = arg.kind else {
            unreachable!("checked input observation")
        };
        return crate::observed::capture(&arg.ty, &format!("self.input{id}"), slot);
    }
    if let Kind::ObservedLocal(id) = v.kind
        && let Ty::Delta(origin) = &v.ty
    {
        return crate::observed::capture(origin, &format!("local{id}"), slot);
    }
    let marker = global_type(&v.ty);
    let id = prelude.len();
    prelude.push(format!("let prepared_item{id}={};", emit(plan, v)));
    format!(
        "<{marker} as hgl_store::PreparedValue>::check_native(columns,{slot},&prepared_item{id})?;<{marker} as hgl_store::PreparedValue>::copy_native(columns,{slot},&prepared_item{id});"
    )
}

pub(crate) fn append(plan: &Plan, item: &Value, list: &str) -> String {
    let marker = global_type(&item.ty);
    let mut prelude = Vec::new();
    let write = write(plan, item, "destination", &mut prelude);
    format!(
        "{{{}let now=_ctx.evaluation_time();let list={list};let mut prepared=_ctx.prepared();let (observation,globals)=prepared.storage.observations();let columns=globals.values_mut();let destination=hgl_store::append_slot::<{marker}>(columns,list)?;{}hgl_store::commit_append::<{marker}>(columns,list);}}",
        prelude.concat(),
        write
    )
}
pub(crate) fn replace(plan: &Plan, item: &Value, slot: &str) -> String {
    let mut prelude = Vec::new();
    let write = write(plan, item, "destination", &mut prelude);
    format!(
        "{{{}let destination={slot};let now=_ctx.evaluation_time();let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();{write}}}",
        prelude.concat()
    )
}

pub(crate) fn set(plan: &Plan, id: usize, v: &Value) -> String {
    if v.ty == Ty::Any && crate::finite_domains::prepared(plan) {
        return format!(
            "{{let destination=_ctx.global_state().destination(self.global{id});{}_ctx.global_state().mark_present(self.global{id});}}",
            crate::any_prepared::write(plan, v, "destination", |v| emit(plan, v))
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
