//! Independent typed tuple locals in prepared ordinary slots.
use crate::layouts::{global_type, rust_type};
use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::fmt::Write as _;
fn append(code: &mut String, args: std::fmt::Arguments<'_>) {
    code.write_fmt(args)
        .unwrap_or_else(|_| unreachable!("String formatting"));
}
fn scan(
    body: &mut Vec<Statement>,
    locals: &mut std::collections::BTreeMap<usize, Ty>,
    next: &mut usize,
) {
    let mut result = Vec::new();
    for mut statement in std::mem::take(body) {
        match &mut statement {
            Statement::Let(id, v) if v.snapshot => {
                locals.insert(*id, v.ty.clone());
            }
            Statement::Return(value)
            | Statement::Assign(
                Value {
                    kind: Kind::Output, ..
                },
                value,
            ) if value.snapshot && matches!(value.kind, Kind::Unary(..) | Kind::Construct(_)) => {
                let id = *next;
                *next += 1;
                locals.insert(id, value.ty.clone());
                result.push(Statement::Let(id, value.clone()));
                let mut local = Value::new(value.ty.clone(), Kind::Local(id));
                local.snapshot = true;
                *value = local;
            }
            Statement::If(_, a, b) => {
                scan(a, locals, next);
                scan(b, locals, next);
            }
            Statement::While(_, b)
            | Statement::For(_, _, b)
            | Statement::ForItems(_, _, _, _, b) => scan(b, locals, next),
            Statement::Exit
            | Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..) => {}
        }
        result.push(statement);
    }
    *body = result;
}
fn maximum(body: &[Statement]) -> usize {
    body.iter()
        .map(|s| match s {
            Statement::Let(id, _) | Statement::Var(id, _) | Statement::Borrow(id, _, _) => id + 1,
            Statement::If(_, a, b) => maximum(a).max(maximum(b)),
            Statement::While(_, b) => maximum(b),
            Statement::For(id, _, b) => (id + 1).max(maximum(b)),
            Statement::ForItems(key, child, _, _, b) => (key + 1).max(child + 1).max(maximum(b)),
            Statement::Exit
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..) => 0,
        })
        .max()
        .unwrap_or(0)
}

/// Reserve private local destinations in the emission plan; no source type is changed.
pub fn prepare(plan: &Plan) -> Plan {
    let mut plan = plan.clone();
    for (node_id, node) in plan.nodes.iter_mut().enumerate() {
        let mut locals = std::collections::BTreeMap::new();
        let mut next = maximum(&node.start).max(maximum(&node.stop)).max(
            node.handlers
                .iter()
                .map(|(_, body)| maximum(body))
                .max()
                .unwrap_or(0),
        );
        scan(&mut node.start, &mut locals, &mut next);
        scan(&mut node.stop, &mut locals, &mut next);
        for (_, body) in &mut node.handlers {
            scan(body, &mut locals, &mut next);
        }
        if !locals.is_empty() {
            node.global_state = true;
        }
        for (id, ty) in locals {
            let key = format!("\0snapshot:{node_id}:{id}");
            if !node.globals.iter().any(|(k, _)| k == &key) {
                node.globals.push((key, crate::snapshots::storage(&ty)));
            }
        }
    }
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
    let marker = global_type(&crate::snapshots::storage(&value.ty));
    // The private destination index is replaced by the caller from its node's exact globals.
    let destination = format!("self.snapshot{id}");
    let write = if let Kind::Unary(op, input) = &value.kind
        && op == "snapshot"
    {
        format!(
            "let input={};let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();if !observation.bindings.valid(input.id()) {{return Err(hgl_types::NodeError::new(\"ordinary tuple input is invalid\"));}}{}",
            crate::observed::input(input),
            capture(&value.ty, "input", "destination")
        )
    } else if matches!(value.kind, Kind::Construct(_)) {
        constructed(value, "destination", &emit)
    } else {
        format!(
            "let source={};let globals=_ctx.global_state();let columns=globals.values_mut();<{marker} as hgl_store::PreparedValue>::check_slots(columns,source,columns,destination)?;<{marker} as hgl_store::PreparedValue>::copy_within(columns,source,destination);",
            projection(value, &emit)
        )
    };
    format!(
        "let local{id}={{let destination=_ctx.global_state().destination({destination});{write}destination}};\n"
    )
}
/// Project optional descendants without erasing their validity; scalars retain only themselves.
pub fn projection(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    if let Kind::Local(id) | Kind::MutableLocal(id) = &value.kind {
        format!("local{id}")
    } else if let Kind::Field(parent, index) = &value.kind {
        format!(
            "{{let parent={};let optional=parent.fields().{index};if _ctx.store().global_values().list(optional.fields()).is_empty() {{return Err(hgl_types::NodeError::new(\"absent ordinary tuple child\"));}}{}}}",
            emit(parent),
            slot(&value.ty, "optional", "_ctx.store().global_values()")
        )
    } else if let Kind::Index(parent, index) = &value.kind {
        let Ty::List(wrapper, _) = crate::snapshots::storage(&parent.ty) else {
            unreachable!("list snapshot");
        };
        format!(
            "{{let parent={};let index={};let item={{let columns=_ctx.store().global_values();let positions=hgl_store::list_index(columns.list(parent.fields()),index)?;hgl_store::ValueSlot::<{}>::bind(&mut positions.as_slice())}};let optional=item.fields().0;if _ctx.store().global_values().list(optional.fields()).is_empty() {{return Err(hgl_types::NodeError::new(\"absent ordinary list child\"));}}{}}}",
            emit(parent),
            emit(index),
            global_type(&wrapper),
            slot(&value.ty, "optional", "_ctx.store().global_values()")
        )
    } else {
        unreachable!("prepared snapshot local")
    }
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
        && !matches!(
            value.ty,
            Ty::Tuple(_) | Ty::Struct(..) | Ty::List(..) | Ty::Map(..)
        ) {
        format!(
            "_ctx.prepared().scalar_from_global::<{}>(self._output.id(),self._output.generation(),publication.fields())?;",
            rust_type(&value.ty)
        )
    } else if value.snapshot {
        publish(&value.ty, "self._output", "publication")
    } else {
        crate::snapshots::publish(&value.ty, "self._output", "publication")
    };
    Some(format!(
        "let publication={publication};{publish}return Ok(());\n"
    ))
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
                format!("let source={};let columns=_ctx.global_state().values_mut();let to={child_slot};<{marker} as hgl_store::PreparedValue>::check_slots(columns,source,columns,to)?;<{marker} as hgl_store::PreparedValue>::copy_within(columns,source,to);columns.set_list_len(({target}).fields(),1);",projection(child,emit))
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
pub fn native_text(value: &Value) -> String {
    let input = crate::observed::input(value);
    let check = if matches!(value.kind, Kind::Input(..)) {
        ""
    } else {
        "if !_ctx.store().input_valid(input.id()) {return Err(hgl_types::NodeError::new(\"ordinary tuple input is invalid\"));}"
    };
    format!("{{let input={input};{check}_ctx.store().get_ref(input).as_str()}}")
}
