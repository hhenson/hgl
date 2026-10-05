use super::{Kind, Plan, Value, global_type, value};
use hgl_source::Ty;
pub(super) fn forward(plan: &Plan, value: &Value) -> Option<String> {
    if let Kind::Configuration(id) = value.kind
        && value.ty.atomic_payload()
    {
        return Some(format!(
            "{}return Ok(());\n",
            hgl_rust_observed::apply(
                &value.ty.clone().atomic(),
                "self._output",
                "&self.configuration_columns",
                &format!("self.configuration_slot{id}")
            )
        ));
    }
    if let Some(code) = hgl_rust_direct_deltas::publish(value, |v| super::value(plan, v)) {
        return Some(code);
    }
    if let Some(code) = hgl_rust_observed::text(value) {
        return Some(code);
    }
    let (ty, input) = if let Kind::Query(op, args) = &value.kind {
        if op != "delta_value" {
            return None;
        }
        let arg = args.first()?;
        let Kind::Input(id, _) = arg.kind else {
            return None;
        };
        (&arg.ty, format!("self.input{id}"))
    } else if let Kind::ObservedLocal(id) = value.kind {
        let Ty::Delta(origin) = &value.ty else {
            return None;
        };
        (origin.as_ref(), format!("local{id}"))
    } else {
        return None;
    };
    Some(format!(
        "{}return Ok(());\n",
        hgl_rust_observed::pass(ty, &input, "self._output")
    ))
}
fn write(plan: &Plan, v: &Value, slot: &str, prelude: &mut Vec<String>) -> String {
    if let Kind::Construct(fields) = &v.kind {
        return fields
            .iter()
            .map(|(i, value)| write(plan, value, &format!("({slot}).fields().{i}"), prelude))
            .collect();
    }
    if let Kind::Query(op, args) = &v.kind
        && op == "delta_value"
    {
        let arg = &args[0];
        let Kind::Input(id, _) = arg.kind else {
            unreachable!("checked input observation")
        };
        return hgl_rust_observed::capture(&arg.ty, &format!("self.input{id}"), slot);
    }
    if let Kind::ObservedLocal(id) = v.kind
        && let Ty::Delta(origin) = &v.ty
    {
        return hgl_rust_observed::capture(origin, &format!("local{id}"), slot);
    }
    let marker = global_type(&v.ty);
    let id = prelude.len();
    prelude.push(format!("let prepared_item{id}={};", value(plan, v)));
    format!(
        "<{marker} as hgl_store::PreparedValue>::check_native(columns,{slot},&prepared_item{id})?;<{marker} as hgl_store::PreparedValue>::copy_native(columns,{slot},&prepared_item{id});"
    )
}

pub(super) fn append(plan: &Plan, item: &Value, list: &str) -> String {
    let marker = global_type(&item.ty);
    let mut prelude = Vec::new();
    let write = write(plan, item, "destination", &mut prelude);
    format!(
        "{{{}let now=_ctx.evaluation_time();let list={list};let mut prepared=_ctx.prepared();let (observation,globals)=prepared.storage.observations();let columns=globals.values_mut();let destination=hgl_store::append_slot::<{marker}>(columns,list)?;{}hgl_store::commit_append::<{marker}>(columns,list);}}",
        prelude.concat(),
        write
    )
}

pub(super) fn set(plan: &Plan, id: usize, v: &Value) -> String {
    if !hgl_rust_finite_domains::prepared(plan)
        || plan.recording.as_ref().is_none_or(|(_, ty)| ty != &v.ty)
    {
        return format!(
            "{{let value={};_ctx.global_state().set(self.global{id},&value)?;}}",
            value(plan, v)
        );
    }
    let marker = global_type(&v.ty);
    format!(
        "{{let value={};let globals=_ctx.global_state();let slot=globals.destination(self.global{id});<{marker} as hgl_store::PreparedValue>::check_native(globals.values(),slot,&value)?;<{marker} as hgl_store::PreparedValue>::copy_native(globals.values_mut(),slot,&value);globals.mark_present(self.global{id});}}",
        value(plan, v)
    )
}
