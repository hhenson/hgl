//! Independent typed tuple locals in prepared ordinary slots.
use crate::append;
use crate::layouts::{global_type, rust_type};
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::Ty;

/// Reserve private destinations before emitting typed local writes.
pub fn prepare(plan: &Plan) -> Plan {
    let mut plan = crate::any_prepared::prepare(plan);
    hgl_semantics::snapshot_locals::prepare(&mut plan, crate::snapshots::storage);
    plan
}
fn slot(ty: &Ty, fields: &str, columns: &str) -> String {
    format!(
        "hgl_store::ValueSlot::<{}>::bind(&mut {columns}.prepared_list(({fields}).fields()).first().ok_or_else(||hgl_types::NodeError::new(\"unprepared snapshot child\"))?.as_slice())",
        global_type(&crate::snapshots::storage(ty))
    )
}
fn element(ty: &Ty, parent: &str, index: &str, columns: &str) -> String {
    format!(
        "hgl_store::ValueSlot::<{}>::bind(&mut {columns}.prepared_list(({parent}).fields())[{index}].as_slice())",
        global_type(ty)
    )
}
fn capture_child(ty: &Ty, input: &str, destination: &str) -> String {
    format!(
        "{{let input={input};let optional={destination};let present=observation.bindings.valid(input.id());if present {{let destination={};{}}}columns.set_list_len(optional.fields(),usize::from(present));}}",
        slot(ty, "optional", "columns"),
        capture(ty, "input", "destination")
    )
}
fn capture(ty: &Ty, input: &str, destination: &str) -> String {
    if let Some(children) = crate::snapshots::children(ty) {
        return children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                capture_child(
                    child,
                    &format!("({input}).field::<{i}>(observation.bindings)"),
                    &format!("({destination}).fields().{i}"),
                )
            })
            .collect();
    }
    if let Ty::List(child, size) = ty {
        let n = size.map_or_else(
            || format!("observation.bindings.input(({input}).id()).members.live.len()"),
            |n| n.to_string(),
        );
        let input_child = if size.is_some() {
            format!("({input}).index(observation.bindings,index)")
        } else {
            format!(
                "({input}).member(observation.bindings,index as i64).ok_or_else(||hgl_types::NodeError::new(\"missing growing list child\"))?"
            )
        };
        let layout = crate::snapshots::storage(ty);
        let Ty::List(wrapper, _) = layout else {
            unreachable!()
        };
        return format!(
            "{{let count={n};let destination={destination};if columns.prepared_list(destination.fields()).len()<count {{return Err(hgl_types::NodeError::new(\"snapshot list capacity exceeded\"));}}for index in 0..count {{let item={};{}}}columns.set_list_len(destination.fields(),count);}}",
            element(&wrapper, "destination", "index", "columns"),
            capture_child(child, &input_child, "item.fields().0")
        );
    }
    if let Ty::Map(key, child) = ty {
        let storage = crate::snapshots::storage(ty);
        let pair = Ty::Tuple(vec![
            (**key).clone(),
            if let Ty::Map(_, child) = &storage {
                (**child).clone()
            } else {
                unreachable!()
            },
        ]);
        let marker = global_type(key);
        return format!(
            "{{let destination={destination};let count=observation.bindings.input(({input}).id()).members.live.len();if columns.prepared_list(destination.fields()).len()<count {{return Err(hgl_types::NodeError::new(\"snapshot map capacity exceeded\"));}}let mut index=0;let mut any=false;for &(key,_) in &observation.bindings.input(({input}).id()).members.prepared {{if let Some(input)=({input}).member(observation.bindings,key) {{let item={};<{marker} as hgl_store::Key>::with_value(observation.keys,key,|value|->hgl_types::NodeResult {{<{marker} as hgl_store::PreparedValue>::check_native(columns,item.fields().0,value)?;<{marker} as hgl_store::PreparedValue>::copy_native(columns,item.fields().0,value);Ok(())}})??;{}any|=!columns.list(item.fields().1.fields().0.fields()).is_empty();index+=1;}}}}if count==0||!any {{return Err(hgl_types::NodeError::new(\"ordinary tuple input is invalid\"));}}if index!=count {{return Err(hgl_types::NodeError::new(\"snapshot map requires prepared key domain\"));}}columns.set_list_len(destination.fields(),count);}}",
            element(&pair, "destination", "index", "columns"),
            capture_child(child, "input", "item.fields().1.fields().0")
        );
    }
    let marker = global_type(ty);
    let scalar = rust_type(ty);
    format!(
        "{{let value=observation.scalar::<{scalar}>(({input}).id())?;<{marker} as hgl_store::PreparedValue>::check_native(columns,{destination},value)?;<{marker} as hgl_store::PreparedValue>::copy_native(columns,{destination},value);}}"
    )
}
/// Initialize one independent prepared local from the current temporal value or another owner.
pub fn local(id: usize, value: &Value, emit: impl Fn(&Value) -> String) -> String {
    // The private destination index is replaced by the caller from its node's exact globals.
    let destination = format!("self.snapshot{id}");
    let write = if let Kind::Unary(op, input) = &value.kind
        && op == "snapshot"
    {
        format!(
            "let input={};let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();if !observation.bindings.valid(input.id()) {{return Err(hgl_types::NodeError::new(\"ordinary tuple input is invalid\"));}}{}true",
            crate::observed::input(input),
            if matches!(input.ty, Ty::Rolling(..)) {
                crate::observed::capture(&input.ty, "input", "destination")
            } else {
                capture(&value.ty, "input", "destination")
            }
        )
    } else if matches!(value.kind, Kind::Construct(_)) {
        format!("{}true", constructed(value, "destination", &emit))
    } else {
        crate::snapshot_views::copy(value, "destination", &emit)
    };
    format!(
        "let local{id}={{let destination=_ctx.global_state().destination({destination});let present={{{write}}};(destination,present)}};\n"
    )
}
/// Project optional descendants without erasing their validity; scalars retain only themselves.
pub fn projection(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    crate::snapshot_views::payload(value, emit)
}
/// Retain a scalar projection; aggregate projections remain independently owned slots.
pub fn read(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    let source = projection(value, emit);
    if matches!(
        value.ty,
        Ty::Tuple(_) | Ty::Struct(..) | Ty::List(..) | Ty::Map(..)
    ) {
        source
    } else {
        format!("({source}).read(_ctx.store().global_values())?")
    }
}
/// Publish a complete retained value through the structural publication owner.
pub fn publish(ty: &Ty, output: &str, source: &str) -> String {
    crate::structural_publication::prepared(ty, output, source)
}
/// Select the complete ordinary tuple publication boundary before ordinary scalar transport.
pub fn returned(value: &Value, expression: &str, result: Option<&Ty>) -> Option<String> {
    publication(value, expression, result).map(|code| format!("{code}return Ok(());\n"))
}
/// Select exact prepared scalar, structural or rolling publication transport.
pub fn publication(value: &Value, expression: &str, result: Option<&Ty>) -> Option<String> {
    if result.is_some_and(|ty| matches!(ty, Ty::Atomic(_)))
        || (!value.snapshot && !matches!(value.ty, Ty::Tuple(_) | Ty::Struct(..)))
    {
        return None;
    }
    let publication = if value.snapshot {
        expression.into()
    } else {
        crate::snapshots::complete(&value.ty, expression)
    };
    let publish = if value.snapshot
        && matches!(value.ty, Ty::Any | Ty::Bytes)
        && result.is_some_and(|ty| matches!(ty, Ty::Rolling(..)))
    {
        "_ctx.prepared().rolling_from(None,publication,self._output)?;".into()
    } else if value.snapshot && value.ty == Ty::Any {
        "_ctx.prepared().any_from_global(self._output,publication)?;".into()
    } else if value.snapshot
        && !matches!(
            value.ty,
            Ty::Tuple(_) | Ty::Struct(..) | Ty::List(..) | Ty::Map(..)
        )
    {
        format!(
            "_ctx.prepared().scalar_from_global::<{}>(self._output.id(),self._output.generation(),publication.fields())?;",
            rust_type(&value.ty)
        )
    } else if value.snapshot {
        publish(&value.ty, "self._output", "publication")
    } else {
        crate::snapshots::publish(&value.ty, "self._output", "publication")
    };
    Some(format!("let publication={publication};{publish}"))
}

fn constructed(value: &Value, destination: &str, emit: &impl Fn(&Value) -> String) -> String {
    let Kind::Construct(fields) = &value.kind else {
        unreachable!("tuple construction");
    };
    fields.iter().fold(String::new(),|mut code,(i, child)| {
        let target = "constructor_optional";
        let child_slot = slot(&child.ty, target, "columns");
        let marker = global_type(&crate::snapshots::storage(&child.ty));
        let write = if child.snapshot {
            if let Kind::Unary(op,input) = &child.kind && op == "snapshot" {
                format!("let input={};let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();let destination={child_slot};if !observation.bindings.valid(input.id()) {{return Err(hgl_types::NodeError::new(\"ordinary tuple input is invalid\"));}}{}columns.set_list_len(({target}).fields(),1);",crate::observed::input(input),capture(&child.ty,"input","destination"))
            } else if matches!(child.kind,Kind::Construct(_)) {
                format!("let destination={{let globals=_ctx.global_state();let columns=globals.values();{child_slot}}};{} _ctx.global_state().values_mut().set_list_len(({target}).fields(),1);",constructed(child,"destination",emit))
            } else {
                crate::snapshot_views::constructor(child, target, emit)
            }
        } else {
            let native = crate::snapshots::complete(&child.ty,&emit(child));
            format!("let value=Some({native});let columns=_ctx.global_state().values_mut();<hgl_store::Optional<{marker}> as hgl_store::PreparedValue>::check_native(columns,{target},&value)?;<hgl_store::Optional<{marker}> as hgl_store::PreparedValue>::copy_native(columns,{target},&value);")
        };
        append(&mut code,format_args!("{{let constructor_optional=({destination}).fields().{i};{write}}}"));code
    })
}

/// Read a statically projected runtime tuple child using its existing typed transport.
pub fn endpoint_read(value: &Value) -> String {
    let input = crate::observed::input(value);
    let read = if crate::layouts::whole_payload(&value.ty).is_some()
        || matches!(
            value.ty,
            Ty::List(..) | Ty::Map(..) | Ty::Tuple(_) | Ty::Struct(..) | Ty::Rolling(..)
        ) {
        crate::observed::read(&value.ty, "input", None)
    } else {
        "_ctx.get(input)".into()
    };
    format!(
        "{{let input={input};if !_ctx.store().input_valid(input.id()) {{return Err(hgl_types::NodeError::new(\"ordinary tuple input is invalid\"));}}{read}}}"
    )
}
/// Borrow scalar text from an endpoint, preserving required child-read validity.
pub fn native_scalar(value: &Value) -> String {
    let input = crate::observed::input(value);
    let check = if matches!(value.kind, Kind::Input(..)) {
        ""
    } else {
        "if !_ctx.store().input_valid(input.id()) {return Err(hgl_types::NodeError::new(\"ordinary tuple input is invalid\"));}"
    };
    format!("{{let input={input};{check}_ctx.store().get_ref(input)}}")
}

/// Borrow text with the same validity checks as every owning scalar endpoint.
pub fn native_text(value: &Value) -> String {
    format!("({}).as_str()", native_scalar(value))
}
