//! Endpoint operations and scalar payload boundaries, separate from graph construction.
use hgl_rust_ir::{Kind, Value};
use hgl_source::{Expr, Literal, Ty};
use hgl_value_bind::method_arguments;
use std::collections::BTreeMap;
type Env = BTreeMap<String, Value>;
type Arguments = Vec<(Option<String>, Value)>;
/// Check set call within the admitted endpoint profile.
pub fn set_call(
    name: &str,
    owner: &str,
    item: &str,
    args: Vec<(Option<String>, Value)>,
) -> Result<Value, String> {
    let op = if owner == "hgraph.native" { item } else { name };
    let first = args
        .first()
        .ok_or("set operation needs an endpoint")?
        .1
        .clone();
    let Ty::Set(element) = &first.ty else {
        return Err("set operation needs a set".into());
    };
    if !matches!(element.as_ref(), Ty::I64 | Ty::Bool) {
        return Err("set keys currently require bool or i64".into());
    }
    if matches!(op, "upsert" | "discard") && !matches!(first.kind, Kind::Output) {
        return Err("set mutation requires out".into());
    }
    if matches!(op, "bound" | "len" | "contains") && !matches!(first.kind, Kind::Input(..)) {
        return Err("set observation requires an input".into());
    }
    let n = if matches!(op, "bound" | "len") { 1 } else { 2 };
    if args.len() != n || (n == 2 && args[1].1.ty != **element) {
        return Err("set argument type mismatch".into());
    }
    let ty = match op {
        "bound" | "contains" => Ty::Bool,
        "len" => Ty::I64,
        _ => Ty::Void,
    };
    Ok(Value {
        ty,
        kind: Kind::Query(
            format!("set_{op}"),
            args.into_iter().map(|(_, v)| v).collect(),
        ),
    })
}
/// Check scalar within the admitted endpoint profile.
pub fn scalar(ty: &Ty) -> bool {
    !matches!(
        ty,
        Ty::Void
            | Ty::Atomic(_)
            | Ty::Ref(_)
            | Ty::Set(_)
            | Ty::Nullable(_)
            | Ty::Struct(..)
            | Ty::List(..)
            | Ty::Map(..)
            | Ty::Tuple(_)
            | Ty::Delta(_)
    )
}
/// Check endpoint metadata within the admitted endpoint profile.
pub fn endpoint_metadata(name: &str) -> bool {
    matches!(
        name,
        "valid" | "modified" | "last_modified" | "activate" | "passivate"
    )
}
/// Check injected clock within the admitted endpoint profile.
pub fn injected_clock(expr: &Expr, env: &Env) -> bool {
    matches!(expr, Expr::Name(name) if name == "clock" && !env.get(name).is_some_and(|v| matches!(v.ty, Ty::Struct(..))))
}
/// Check clock property within the admitted endpoint profile.
pub fn clock_property(
    receiver: &Expr,
    name: &str,
    env: &Env,
    runtime: bool,
) -> Result<Value, String> {
    if !matches!(receiver, Expr::Name(receiver) if receiver == "clock") || !runtime {
        return Err("clock property requires the direct injected clock in a runtime hook".into());
    }
    capability_payload("clock", env)?;
    match name {
        "evaluation_time" | "next_cycle_evaluation_time" => Ok(Value::new(
            Ty::DateTime,
            Kind::Query(format!("clock.{name}"), Vec::new()),
        )),
        "now" => Err("clock.now: wall-clock observations are not supported by this backend".into()),
        _ => Err(format!("clock: unknown property {name}")),
    }
}
/// Check require payload within the admitted endpoint profile.
pub fn require_payload(value: &Value) -> Result<(), String> {
    if matches!(value.kind, Kind::Input(_, true)) {
        return Err("signal has no ordinary scalar value".into());
    }
    if matches!(value.ty, Ty::Nullable(_)) {
        return Err("nullable replay result requires presence proof before payload use".into());
    }
    if matches!(value.kind, Kind::Input(..) | Kind::Output)
        && matches!(
            value.ty,
            Ty::Atomic(_) | Ty::Map(..) | Ty::Tuple(_) | Ty::List(..) | Ty::Struct(..)
        )
    {
        return Err("structural endpoint payload requires delta_value observation".into());
    }
    Ok(())
}
/// Check endpoint call within the admitted endpoint profile.
pub fn endpoint_call(name: &str, args: Arguments, runtime: bool) -> Result<Value, String> {
    if !runtime
        || args.is_empty()
        || args
            .iter()
            .any(|(n, v)| n.is_some() || !matches!(v.kind, Kind::Input(..) | Kind::Output))
    {
        return Err("endpoint query requires runtime endpoints".into());
    }
    if matches!(name, "last_modified" | "activate" | "passivate") && args.len() != 1 {
        return Err("endpoint operation requires exactly one argument".into());
    }
    if matches!(name, "activate" | "passivate")
        && args
            .iter()
            .any(|(_, v)| !matches!(v.kind, Kind::Input(..)) || !scalar(&v.ty))
    {
        return Err("activity requires a scalar input".into());
    }
    if args
        .iter()
        .any(|(_, v)| matches!(v.kind, Kind::Output) && !scalar(&v.ty))
    {
        return Err("querying structural out is not yet supported".into());
    }
    let ty = match name {
        "last_modified" => Ty::DateTime,
        "activate" | "passivate" => Ty::Void,
        _ => Ty::Bool,
    };
    Ok(Value::new(
        ty,
        Kind::Query(name.to_owned(), args.into_iter().map(|(_, v)| v).collect()),
    ))
}

/// Resolve an injected capability marker without reading a payload.
pub fn capability_payload(receiver: &str, env: &Env) -> Result<Ty, String> {
    env.get(receiver)
        .filter(|v| matches!(v.kind, Kind::Capability))
        .map(|v| v.ty.clone())
        .ok_or_else(|| format!("{receiver}: missing inject {receiver}"))
}

/// Check an injected operation without owning graph phase or construction.
pub fn capability_function(
    receiver: &str,
    method: &str,
    args: &Arguments,
    env: &Env,
    stopping: bool,
) -> Result<Value, String> {
    let name = format!("{method}({receiver})");
    capability_payload(receiver, env)?;
    let (parameters, result) = match (receiver, method) {
        ("alarm", "schedule") if !stopping => (vec![("delay", Ty::Duration)], Ty::Void),
        ("alarm", "schedule_at") if !stopping => (vec![("time", Ty::DateTime)], Ty::Void),
        _ => {
            return Err(format!(
                "{name}: unknown capability operation or forbidden hook phase"
            ));
        }
    };
    let values = method_arguments(&name, &parameters, args)?;
    if receiver == "alarm"
        && method == "schedule"
        && matches!(values[0].kind, Kind::Literal(Literal::Duration(delay)) if delay < 0)
    {
        return Err("negative alarm delay".into());
    }
    Ok(Value::new(
        result,
        Kind::Query(format!("{receiver}.{method}"), values),
    ))
}
