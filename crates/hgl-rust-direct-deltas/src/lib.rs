//! Retained constant aliases and direct sparse publication without owning intermediaries.
mod normalize;
use hgl_rust_ir::{DeltaEntry, Kind, Value};
use hgl_rust_layouts::{global_type, rust_type};
use hgl_source::Ty;
pub use normalize::prepare;
fn scalar(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::I64
            | Ty::F64
            | Ty::Bool
            | Ty::Str
            | Ty::Duration
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::CivilDateTime
            | Ty::TimeZone
            | Ty::ZonedTime
            | Ty::ZonedDateTime
    )
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
        || scalar(&value.ty)
            && match &value.kind {
                Kind::Input(..) | Kind::Configuration(_) | Kind::Literal(_) => true,
                Kind::Binary(_, a, b) => !matches!(value.ty, Ty::Str) && payload(a) && payload(b),
                Kind::Unary(_, child) => payload(child),
                Kind::Delta(_)
                | Kind::ObservedLocal(_)
                | Kind::WiringFailure(_)
                | Kind::List(_)
                | Kind::Index(..)
                | Kind::Length(_)
                | Kind::Push(..)
                | Kind::ValueCall(..)
                | Kind::Construct(_)
                | Kind::Field(..)
                | Kind::GlobalGet(_)
                | Kind::BorrowedLocal(..)
                | Kind::GlobalSet(..)
                | Kind::IsPresent(_)
                | Kind::Present(_)
                | Kind::TemporalLiteral(_)
                | Kind::Prepared(_)
                | Kind::Wire(_)
                | Kind::Cache(_)
                | Kind::GeneratorLocal(_)
                | Kind::Local(_)
                | Kind::MutableLocal(_)
                | Kind::Native(..)
                | Kind::Query(..)
                | Kind::Output
                | Kind::Capability
                | Kind::Void => false,
            }
}
/// Emit one supported returned constructor, evaluating all scalar operands before mutation.
pub fn publish(value: &Value, emit: impl Fn(&Value) -> String) -> Option<String> {
    if !supported(value) {
        return None;
    }
    let mut prelude = Vec::new();
    let body = delta(value, "self._output", &emit, &mut prelude);
    Some(format!("{}{}return Ok(());", prelude.concat(), body))
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
        return hgl_rust_observed::apply(
            &value.ty,
            output,
            "&self.configuration_columns",
            &format!("self.configuration_slot{id}"),
        );
    }
    if let Kind::Input(id, _) = value.kind {
        return hgl_rust_observed::pass(&value.ty, &format!("self.input{id}"), output);
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
    if parts.is_empty() {
        return "return Err(hgl_types::NodeError::new(\"empty structural delta application is outside the supported profile\"));".into();
    }
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
    validate.concat() + &removed.concat() + &updates.concat()
}
