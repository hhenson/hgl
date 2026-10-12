//! Statically specialized sparse delta construction, observation and publication.
use crate::append;
use crate::layouts::{
    delta_storage, delta_type, global_type, owned_type, rust_type, whole_payload,
};
use hgl_semantics::ir::{DeltaEntry, Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeSet;

/// Whether a temporal shape uses structural publication data.
pub fn structural(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Set(_) | Ty::List(..) | Ty::Map(..) | Ty::Tuple(_) | Ty::Struct(..)
    )
}
fn identity(ty: &Ty) -> String {
    ty.source_name()
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .concat()
}
/// Compile-time marker for one exact prepared temporal shape.
pub fn shape_marker(ty: &Ty) -> String {
    if matches!(ty, Ty::Rolling(..)) {
        crate::windows::marker(ty)
    } else if matches!(ty, Ty::Enum(_) | Ty::Native(_) | Ty::Any) {
        format!("hgl_store::shapes::Atomic<{}>", global_type(ty))
    } else if let Ty::Atomic(payload) = ty {
        format!("hgl_store::shapes::Atomic<{}>", global_type(payload))
    } else if let Ty::List(child, size) = ty {
        size.map_or_else(
            || format!("hgl_store::shapes::Growing<{}>", shape_marker(child)),
            |n| format!("hgl_store::shapes::Fixed<{}, {n}>", shape_marker(child)),
        )
    } else if let Ty::Map(key, child) = ty {
        format!(
            "hgl_store::shapes::Map<{},{}>",
            shape_marker(child),
            global_type(key)
        )
    } else if let Ty::Set(key) = ty {
        format!("hgl_store::shapes::Set<{}>", global_type(key))
    } else if matches!(ty, Ty::Tuple(_) | Ty::Struct(..)) {
        format!("Shape{}", identity(ty))
    } else {
        rust_type(ty)
    }
}
fn operations(ty: &Ty) -> String {
    global_type(&delta_type(ty))
}
/// Emit publication of an already retained owning payload.
pub fn publish(ty: &Ty, payload: &str) -> String {
    if matches!(
        ty,
        Ty::Any
            | Ty::Enum(_)
            | Ty::Native(_)
            | Ty::Recursive(_)
            | Ty::Family(_)
            | Ty::List(..)
            | Ty::Set(_)
            | Ty::Map(..)
            | Ty::Tuple(_)
            | Ty::Struct(..)
    ) {
        return format!(
            "_ctx.set_atomic::<{}>(self._output,{payload})?;",
            global_type(ty)
        );
    }
    if let Ty::Delta(origin) = ty {
        format!(
            "{}::apply(self._output,{payload},_ctx)?;",
            operations(origin)
        )
    } else {
        format!("_ctx.set(self._output,{payload});")
    }
}
/// Retain exactly the sparse current observation of a prepared input token.
pub fn observe(ty: &Ty, input: &str) -> String {
    if matches!(ty, Ty::Enum(_)) {
        return read(ty, input);
    }
    let Ty::Delta(origin) = ty else {
        unreachable!("structural observation")
    };
    read(origin, input)
}
/// Read one checked child endpoint using its statically selected transport.
pub fn read(ty: &Ty, input: &str) -> String {
    crate::observed::read(ty, input, structural(ty).then(|| operations(ty)))
}

fn apply(ty: &Ty, output: &str, payload: &str) -> String {
    if matches!(ty, Ty::Rolling(..)) {
        return crate::windows::apply(ty, output, payload);
    }
    if let Some(ty) = whole_payload(ty) {
        return format!(
            "_ctx.set_atomic::<{}>({output},{payload})?;",
            global_type(ty)
        );
    }
    if structural(ty) {
        format!("{}::apply({output},{payload},_ctx)?;", operations(ty))
    } else {
        format!("_ctx.set(hgl_store::Store::prepared_output({output}),{payload});")
    }
}
fn allocation(ty: &Ty) -> String {
    if matches!(ty, Ty::Rolling(..)) {
        return format!(
            "store.add_shaped_output(owner,<{} as hgl_store::shapes::Shape>::shape())",
            shape_marker(ty)
        );
    }
    if let Some(payload) = whole_payload(ty) {
        return format!("store.add_atomic_output::<{}>(owner)", global_type(payload));
    }
    if structural(ty) {
        format!("{}::allocate(store,owner)", operations(ty))
    } else {
        format!("store.add_output::<{}>(owner).id()", rust_type(ty))
    }
}
fn children(ty: &Ty) -> Vec<&Ty> {
    if let Ty::Struct(_, fields, _) = ty {
        return fields.iter().map(|(_, t)| t).collect();
    }
    if let Ty::Tuple(children) = ty {
        return children.iter().collect();
    }
    Vec::new()
}
fn empty(ty: &Ty) -> String {
    let Ty::Struct(_, fields, _) = delta_storage(ty) else {
        unreachable!()
    };
    if fields.is_empty() {
        "()".into()
    } else {
        format!("({},)", vec!["Vec::new()"; fields.len()].join(","))
    }
}
fn reserve(target: &str) -> String {
    format!("{target}.try_reserve(1).map_err(|e|hgl_types::NodeError::new(e.to_string()))?;")
}
/// Preserve written constructor entry order before assembling sparse storage.
pub fn construct(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    let Ty::Delta(origin) = &value.ty else {
        unreachable!()
    };
    let Kind::Delta(entries) = &value.kind else {
        unreachable!()
    };
    let mut code = format!(
        "{{ let mut delta: {} = {};",
        owned_type(&value.ty),
        empty(origin)
    );
    for entry in entries {
        match entry {
            DeltaEntry::Add(key) | DeltaEntry::Remove(key) => {
                let slot = if matches!(entry, DeltaEntry::Add(_)) {
                    0
                } else if matches!(origin.as_ref(), Ty::Map(..) | Ty::List(_, None)) {
                    2
                } else {
                    1
                };
                code += &reserve(&format!("delta.{slot}"));
                append(&mut code, format_args!("delta.{slot}.push({});", emit(key)));
            }
            DeltaEntry::Keyed(key, value) => {
                append(
                    &mut code,
                    format_args!("let key={};let child={};", emit(key), emit(value)),
                );
                code += &reserve("delta.0");
                code += &reserve("delta.1");
                code += "delta.0.push(key);delta.1.push(child);";
            }
            DeltaEntry::Child(key, value) => {
                append(&mut code, format_args!("let child = {};", emit(value)));
                if matches!(origin.as_ref(), Ty::List(..) | Ty::Map(..)) {
                    code += &reserve("delta.0");
                    code += &reserve("delta.1");
                    append(
                        &mut code,
                        format_args!("delta.0.push({key}_i64);delta.1.push(child);"),
                    );
                } else {
                    code += &reserve(&format!("delta.{key}"));
                    append(&mut code, format_args!("delta.{key}.push(child);"));
                }
            }
        }
    }
    code + "delta }"
}
/// Emit static shape proofs and sparse operations reached by a checked plan.
pub fn markers(plan: &Plan) -> String {
    let mut types = BTreeSet::new();
    for node in &plan.nodes {
        origin(&node.result, &mut types);
        for (_, _, ty) in &node.inputs {
            origin(ty, &mut types);
        }
        for (_, ty) in &node.globals {
            collect(ty, &mut types);
        }
        for value in &node.configuration {
            values(value, &mut types);
        }
        statements(&node.start, &mut types);
        statements(&node.stop, &mut types);
        if let Some(body) = &node.generator {
            statements(body, &mut types);
        }
        for (guard, body) in &node.handlers {
            if let Some(guard) = guard {
                values(guard, &mut types);
            }
            statements(body, &mut types);
        }
    }
    types.iter().map(marker).collect()
}
fn origin(ty: &Ty, types: &mut BTreeSet<Ty>) {
    if let Ty::Ref(child) = ty {
        origin(child, types);
        return;
    }
    if !structural(ty) || !ty.publication() {
        return;
    }
    types.insert(ty.clone());
    if let Ty::List(child, _) | Ty::Map(_, child) = ty {
        origin(child, types);
    }
    if let Ty::Struct(_, fields, _) = ty {
        for (_, child) in fields {
            origin(child, types);
        }
    }
    if let Ty::Tuple(children) = ty {
        for child in children {
            origin(child, types);
        }
    }
}

fn collect(ty: &Ty, types: &mut BTreeSet<Ty>) {
    if let Ty::Delta(ty) | Ty::Ref(ty) = ty {
        origin(ty, types);
    } else if let Ty::List(child, _) = ty {
        collect(child, types);
    } else if let Ty::Struct(_, fields, _) = ty {
        for (_, child) in fields {
            collect(child, types);
        }
    }
}

fn values(value: &Value, types: &mut BTreeSet<Ty>) {
    hgl_semantics::ir_visit::value(value, &mut |value| {
        collect(&value.ty, types);
        !matches!(value.kind, Kind::Captured(..))
    });
}
fn statements(body: &[Statement], types: &mut BTreeSet<Ty>) {
    hgl_semantics::ir_visit::statements(body, &mut |value| {
        collect(&value.ty, types);
        !matches!(value.kind, Kind::Captured(..))
    });
}

mod emit;
use emit::marker;

/// Whether endpoint handles carry an exact prepared shape rather than a scalar column.
pub fn shaped(ty: &Ty) -> bool {
    structural(ty) || whole_payload(ty).is_some() || matches!(ty, Ty::Rolling(..))
}

/// Lower already-checked collection operands in source order.
pub fn collection_operation(
    op: &str,
    args: &[Value],
    emit: impl FnMut(&Value) -> String,
) -> String {
    crate::keyed::collection_operation(op, args, emit, allocation, apply)
}
