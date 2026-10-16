//! Prepared publication and ordered global hook writes share typed source slots.
use crate::layouts::global_type;
pub(crate) use crate::prepared_global_writes::set;
use crate::values::condition_code as emit;
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::Ty;
pub(crate) fn publication(plan: &Plan, value: &Value, result: Option<&Ty>) -> Option<String> {
    if !crate::finite_domains::prepared(plan) || plan.recording.is_none() {
        return None;
    }
    if let Some(code) = scalar_publication(plan, value, result) {
        return Some(code);
    }
    if let Kind::Query(op, _) = &value.kind
        && op.starts_with("\0constructor:")
        && let Some(("global_values", source)) = crate::source_slots::source(plan, value)
    {
        if matches!(result, Some(Ty::Atomic(_) | Ty::Result(..) | Ty::Error)) {
            return Some(format!(
                "let publication={source};_ctx.prepared().atomic_from_global(publication,self._output)?;"
            ));
        }
        if matches!(value.ty, Ty::Tuple(_))
            && result == Some(&value.ty)
            && let Some(code) =
                crate::structural_publication::ordinary(&value.ty, "self._output", "publication")
        {
            return Some(format!("let publication={source};{code}"));
        }
    }
    if value.snapshot && matches!(value.ty, Ty::Native(_)) {
        let slot = crate::snapshot_slots::projection(value, |v| emit(plan, v));
        if let Some(ty @ Ty::Rolling(..)) = result {
            return Some(crate::windows::from_global(ty, "self._output", &slot));
        }
        return Some(format!(
            "_ctx.prepared().atomic_from_global({slot},self._output)?;"
        ));
    }
    if let Some(code) = crate::native_results::publication(value) {
        return Some(code);
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
    if matches!(result, Some(Ty::Atomic(_) | Ty::Result(..) | Ty::Error))
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
fn scalar_publication(plan: &Plan, value: &Value, result: Option<&Ty>) -> Option<String> {
    if result == Some(&value.ty)
        && value.ty.scalar_kind().is_some()
        && let Some(("global_values", source)) = crate::source_slots::source(plan, value)
    {
        return Some(format!(
            "let publication={source};_ctx.prepared().scalar_from_global::<{}>(self._output.id(),self._output.generation(),publication.fields())?;",
            global_type(&value.ty)
        ));
    }
    None
}

fn capture(v: &Value, slot: &str) -> Option<String> {
    if let Kind::Input(id, false) = v.kind
        && v.ty.scalar_kind().is_some()
    {
        return Some(crate::observed::capture(
            &v.ty,
            &format!("self.input{id}"),
            slot,
        ));
    }
    if let Kind::Query(op, args) = &v.kind
        && op == "delta_value"
    {
        let arg = &args[0];
        let Kind::Input(id, _) = arg.kind else {
            unreachable!("checked input observation")
        };
        return Some(crate::observed::capture(
            &arg.ty,
            &format!("self.input{id}"),
            slot,
        ));
    }
    if let Kind::ObservedLocal(id) = v.kind
        && let Ty::Delta(origin) = &v.ty
    {
        return Some(crate::observed::capture(
            origin,
            &format!("local{id}"),
            slot,
        ));
    }
    None
}

pub(crate) fn write(plan: &Plan, v: &Value, slot: &str, prelude: &mut Vec<String>) -> String {
    if (!v.snapshot || v.ty.owning_scalar())
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
    if let Some(code) = crate::constructor_fields::fields(
        v,
        slot,
        |value, slot| write(plan, value, slot, prelude),
        str::to_owned,
    ) {
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
    if let Some(code) = capture(v, slot) {
        return code;
    }
    if let Some(code) = crate::observed::text_assignment(v, slot, prelude, &|v| emit(plan, v)) {
        return code;
    }
    if let Some(code) = crate::observed::text_assignment(v, slot, prelude, &|v| emit(plan, v)) {
        return code;
    }
    let marker = global_type(&v.ty);
    let id = prelude.len();
    prelude.push(format!("let prepared_item{id}={};", emit(plan, v)));
    format!(
        "<{marker} as hgl_store::PreparedValue>::check_native(columns,{slot},&prepared_item{id})?;<{marker} as hgl_store::PreparedValue>::copy_native(columns,{slot},&prepared_item{id});"
    )
}

pub(crate) fn storage(code: &str) -> String {
    format!(
        "{{let now=_ctx.evaluation_time();_ctx.with_observations(|observation,globals|->hgl_types::NodeResult{{let columns=globals.values_mut();{code}Ok(())}})?;}}"
    )
}

pub(crate) fn append(plan: &Plan, item: &Value, list: &str) -> String {
    let marker = global_type(&item.ty);
    let mut prelude = Vec::new();
    let write = write(plan, item, "destination", &mut prelude);
    let code = storage(&format!(
        "let destination=hgl_store::append_slot::<{marker}>(columns,list)?;{write}hgl_store::commit_append::<{marker}>(columns,list);"
    ));
    format!("{{{}let list={list};{code}}}", prelude.concat())
}

pub(crate) fn replace(plan: &Plan, item: &Value, slot: &str) -> String {
    let mut prelude = Vec::new();
    let write = write(plan, item, "destination", &mut prelude);
    format!(
        "{{{}let destination={slot};{}}}",
        prelude.concat(),
        storage(&write)
    )
}
