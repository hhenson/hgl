//! Retained constant aliases and direct sparse publication without owning intermediaries.
mod normalize;
use crate::layouts::{global_type, rust_type};
use hgl_semantics::ir::{DeltaEntry, Kind, Value};
use hgl_source::Ty;
pub use normalize::prepare;
fn scalar(ty: &Ty) -> bool {
    ty.scalar_kind().is_some() || *ty == Ty::Any
}

fn key(value: &Value) -> bool {
    scalar(&value.ty) && matches!(value.kind, Kind::Literal(_) | Kind::Configuration(_))
}
/// Whether a returned constructor has a statically typed direct publication lowering.
pub fn supported(value: &Value) -> bool {
    let Kind::Delta(parts) = &value.kind else {
        return false;
    };
    parts.iter().all(|part| match part {
        DeltaEntry::Add(k) | DeltaEntry::Remove(k) => key(k),
        DeltaEntry::Keyed(k, child) => key(k) && payload(child),
        DeltaEntry::Child(_, child) => payload(child),
    })
}
fn payload(value: &Value) -> bool {
    supported(value)
        || scalar(&value.ty) && {
            if let Kind::Binary(_, a, b) = &value.kind {
                value.ty != Ty::Str && payload(a) && payload(b)
            } else if let Kind::Unary(_, child) = &value.kind {
                payload(child)
            } else {
                matches!(
                    value.kind,
                    Kind::Input(..) | Kind::Configuration(_) | Kind::Literal(_)
                )
            }
        }
}
/// Emit one supported returned constructor, evaluating all scalar operands before mutation.
pub fn publish(value: &Value, emit: impl Fn(&Value) -> String) -> Option<String> {
    if !supported(value) {
        return None;
    }
    let mut prelude = Vec::new();
    let body = delta(value, "self._output", &emit, &mut prelude);
    Some(format!("{}{body}", prelude.concat()))
}
fn key_code(value: &Value, emit: &impl Fn(&Value) -> String, prelude: &mut Vec<String>) -> String {
    let id = prelude.len();
    let source = if let Kind::Configuration(index) = value.kind {
        format!("&self.configuration{index}")
    } else {
        format!("&({})", emit(value))
    };
    prelude.push(format!(
        "let direct_key{id}=<{} as hgl_store::Key>::id(&_ctx.store().keys,{source})?;",
        global_type(&value.ty)
    ));
    format!("direct_key{id}")
}
fn child(
    value: &Value,
    output: &str,
    emit: &impl Fn(&Value) -> String,
    prelude: &mut Vec<String>,
) -> String {
    if matches!(value.kind, Kind::Delta(_)) {
        return delta(value, output, emit, prelude);
    }
    if let Kind::Configuration(id) = value.kind {
        return crate::observed::apply(
            &value.ty,
            output,
            "&self.configuration_columns",
            &format!("self.configuration_slot{id}"),
        );
    }
    if let Kind::Input(id, _) = value.kind {
        return crate::observed::pass(&value.ty, &format!("self.input{id}"), output);
    }
    let id = prelude.len();
    prelude.push(format!("let direct_value{id}={};", emit(value)));
    format!(
        "_ctx.prepared().scalar::<{}>({output}.id(),{output}.generation(),&direct_value{id})?;",
        rust_type(&value.ty)
    )
}
fn ensure(output: &str, key: &str, child: &str) -> String {
    format!(
        "_ctx.get_or_create_with({output}.id(),{key},|_,_|unreachable!(\"finite child prepared before start\"));let {child}={output}.member(_ctx.store().bindings(),{key}).ok_or_else(||hgl_types::NodeError::new(\"missing prepared member\"))?;"
    )
}
fn delta(
    value: &Value,
    output: &str,
    emit: &impl Fn(&Value) -> String,
    prelude: &mut Vec<String>,
) -> String {
    let (Ty::Delta(origin), Kind::Delta(parts)) = (&value.ty, &value.kind) else {
        unreachable!("checked direct delta")
    };
    let growing = matches!(origin.as_ref(), Ty::List(_, None));
    let mut appended = Vec::new();
    let mut tail = Vec::new();
    let mut keys = Vec::new();
    let mut validate = Vec::new();
    let mut removed = Vec::new();
    let mut updates = Vec::new();
    for part in parts {
        let id = prelude.len();
        let destination = format!("direct_child{id}");
        match part {
            DeltaEntry::Remove(k) => {
                let key = key_code(k, emit, prelude);
                tail.push(key.clone());
                keys.push(key.clone());
                validate.push(format!(
                    "if {output}.member(_ctx.store().bindings(),{key}).is_none() {{return Err(hgl_types::NodeError::new(\"noncanonical removal\"));}}"
                ));
                removed.push(format!("_ctx.remove_shaped({output}.id(),{key});"));
            }
            DeltaEntry::Add(k) => {
                let key = key_code(k, emit, prelude);
                keys.push(key.clone());
                validate.push(format!(
                    "if {output}.member(_ctx.store().bindings(),{key}).is_some() {{return Err(hgl_types::NodeError::new(\"noncanonical set addition\"));}}"
                ));
                updates.push(format!(
                    "{{{}_ctx.prepared().scalar::<bool>({destination}.id(),{destination}.generation(),&true)?;}}",
                    ensure(output, &key, &destination)
                ));
            }
            DeltaEntry::Keyed(k, v) => {
                let key = key_code(k, emit, prelude);
                keys.push(key.clone());
                let body = child(v, &destination, emit, prelude);
                updates.push(format!("{{{}{body}}}", ensure(output, &key, &destination)));
            }
            DeltaEntry::Child(index, v) => {
                if growing {
                    let key = format!("{index}_i64");
                    appended.push(key.clone());
                    let body = child(v, &destination, emit, prelude);
                    updates.push(format!("{{{}{body}}}", ensure(output, &key, &destination)));
                    continue;
                }
                let project = if matches!(origin.as_ref(), Ty::List(..)) {
                    format!("{output}.index(_ctx.store().bindings(),{index})")
                } else {
                    format!("{output}.field::<{index}>(_ctx.store().bindings())")
                };
                let body = child(v, &destination, emit, prelude);
                updates.push(format!("{{let {destination}={project};{body}}}"));
            }
        }
    }
    for (index, key) in keys.iter().enumerate() {
        for prior in &keys[..index] {
            prelude.push(format!("if {key}=={prior} {{return Err(hgl_types::NodeError::new(\"duplicate or overlapping sparse key\"));}}"));
        }
    }
    let range = if growing {
        format!(
            "hgl_types::validate_growing_distinct(_ctx.store().bindings().output({output}.id()).members.live.len(),[{}].into_iter(),[{}].into_iter()).map_err(hgl_types::NodeError::new)?;",
            appended.join(","),
            tail.join(",")
        )
    } else {
        String::new()
    };
    range
        + &validate.concat()
        + &removed.concat()
        + &updates.concat()
        + &format!("_ctx.prepared().initialize_sparse({output}.id(),{output}.generation());")
}
