//! Bind checked values to concrete ordinary and temporal signatures.
use crate::ir::{Kind, Value};
use crate::library::Signature;
use hgl_source::{Expr, Ty, TypeSpelling};
use std::collections::{BTreeMap, BTreeSet};
type Bound = (Vec<Value>, BTreeMap<String, Ty>, Ty);
/// Check bind at the typed call boundary.
pub fn bind(
    signature: &Signature,
    args: &[(Option<String>, Value)],
    runtime: bool,
    hint: Option<&Ty>,
    declared: &BTreeMap<String, Ty>,
) -> Result<Bound, String> {
    let mut supplied = order_arguments(&signature.parameter_names(), args)?;
    let mut types = declared.clone();
    let mut values = Vec::new();
    let mut parameter_names = BTreeSet::new();
    for (index, p) in signature.parameters.iter().enumerate() {
        if !parameter_names.insert(&p.name) {
            return Err("duplicate parameter".into());
        }
        let value = supplied
            .remove(&index)
            .or_else(|| p.value_default.clone())
            .or_else(|| {
                if let Some(Expr::Literal(l)) = p.default.as_ref().map(Expr::syntax) {
                    Some(Value::new(l.ty(), Kind::Literal(l.clone())))
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("missing argument {}", p.name))?;
        if p.constant && !constant(&value) {
            return Err("fixed argument requires a wiring-time value".into());
        }
        if !runtime
            && !p.constant
            && !signature.value_function
            && !matches!(value.kind, Kind::Wire(_))
        {
            return Err("temporal argument requires a port".into());
        }
        if runtime && matches!(value.kind, Kind::Wire(_) | Kind::Void) {
            return Err("native value argument requires a scalar".into());
        }
        let value = bind_type(signature, p, value, &mut types)?;
        values.push(value);
    }
    if signature.generics.contains(&signature.result)
        && !types.contains_key(&signature.result)
        && let Some(hint) = hint
    {
        types.insert(signature.result.clone(), hint.clone());
    }
    crate::struct_declarations::check_domain(signature.type_domain.as_ref(), &types)?;
    let result = resolve_type(&signature.result, &types).ok_or("unresolved result type")?;
    let result = if signature.value_function && !matches!(result, Ty::Atomic(_)) {
        crate::value_access::project(&result)
    } else {
        result
    };
    if !signature.value_function {
        supported_type(&result)?;
    }
    for (parameter, value) in signature.parameters.iter().zip(&values) {
        if !signature.value_function && !parameter.constant {
            supported_type(&value.ty)?;
        }
    }
    Ok((values, types, result))
}

/// Check supported type at the typed call boundary.
pub fn supported_type(mut ty: &Ty) -> Result<(), String> {
    while let Ty::Ref(child) = ty {
        if **child == Ty::Void {
            return Err("reference requires a temporal type".into());
        }
        ty = child;
    }
    (ty.publication() || matches!(ty, Ty::Void | Ty::Nullable(_)))
        .then_some(())
        .ok_or_else(|| "unsupported temporal publication shape".into())
}

fn bind_type(
    signature: &Signature,
    p: &crate::library::Parameter,
    mut value: Value,
    types: &mut BTreeMap<String, Ty>,
) -> Result<Value, String> {
    if !p.constant
        && p.ty != "signal"
        && !p.ty.starts_with("ref<")
        && matches!(value.kind, Kind::Wire(_))
        && let Ty::Ref(child) = &value.ty
    {
        value.ty = *child.clone();
    }
    let formal =
        p.ty.strip_prefix("ref<")
            .or_else(|| p.ty.strip_prefix("set<"))
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(&p.ty);
    let actual = if p.ty.starts_with("ref<") {
        if let Ty::Ref(child) = &value.ty {
            child.as_ref()
        } else {
            &value.ty
        }
    } else if p.ty.starts_with("set<") {
        if let Ty::Set(child) = &value.ty {
            child.as_ref()
        } else {
            return Err("set argument required".into());
        }
    } else {
        &value.ty
    };
    if signature.generics.contains(&formal.to_owned()) {
        let expected = types
            .entry(formal.to_owned())
            .or_insert_with(|| actual.clone());
        if *expected != *actual
            && (!(p.constant || signature.value_function)
                || crate::value_access::project(expected) != *actual)
        {
            return Err("inconsistent generic inference".into());
        }
    } else if p.ty != "signal"
        && resolve_type(formal, types)
            .or_else(|| {
                let (Ty::Ref(child) | Ty::Set(child)) = resolve_type(&p.ty, types)? else {
                    return None;
                };
                Some(*child)
            })
            .map(|ty| {
                if p.constant || signature.value_function {
                    crate::value_access::project(&ty)
                } else {
                    ty
                }
            })
            != Some(actual.clone())
    {
        return Err(format!("type mismatch for {}", p.name));
    }

    if p.ty.starts_with("ref<") && !matches!(value.ty, Ty::Ref(_)) {
        value.ty = Ty::Ref(Box::new(value.ty));
    }
    Ok(value)
}

/// Check resolve type at the typed call boundary.
pub fn resolve_type(name: &str, types: &BTreeMap<String, Ty>) -> Option<Ty> {
    if let Some(ty) = types.get(name) {
        return Some(ty.clone());
    }
    if let Some(origin) = hgl_source::delta_argument(name) {
        return resolve_type(origin, types)?.delta().ok();
    }
    if let Some((base, args)) = hgl_source::application(name) {
        if base == "atomic" && args.len() == 1 {
            return Some(crate::value_access::project(&resolve_type(args[0], types)?).atomic());
        }
        if base == "tuple" {
            return Some(Ty::Tuple(
                args.into_iter()
                    .map(|s| resolve_type(s, types))
                    .collect::<Option<_>>()?,
            ));
        }
        if base == "map" && args.len() == 2 {
            return Some(Ty::Map(
                Box::new(resolve_type(args[0], types)?),
                Box::new(resolve_type(args[1], types)?),
            ));
        }
    }
    if let Some(t) = name.strip_prefix("ref<").and_then(|s| s.strip_suffix('>')) {
        return Some(Ty::Ref(Box::new(resolve_type(t, types)?)));
    }
    if let Some(t) = name.strip_prefix("set<").and_then(|s| s.strip_suffix('>')) {
        return Some(Ty::Set(Box::new(resolve_type(t, types)?)));
    }
    if let Some((element, size)) = Ty::list_parts(name) {
        return Some(Ty::List(Box::new(resolve_type(element, types)?), size));
    }
    Ty::parse(name)
}

/// Check method arguments at the typed call boundary.
pub fn method_arguments(
    name: &str,
    parameters: &[(&str, Ty)],
    args: &[(Option<String>, Value)],
) -> Result<Vec<Value>, String> {
    let names = parameters.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    let bound = order_arguments(&names, args).map_err(|e| format!("{name}: {e}"))?;
    if bound.len() != parameters.len() || bound.iter().any(|(i, v)| parameters[*i].1 != v.ty) {
        return Err(format!("{name}: missing or wrong-type argument"));
    }
    Ok(bound.into_values().collect())
}
/// Check order arguments at the typed call boundary.
pub fn order_arguments(
    parameters: &[&str],
    args: &[(Option<String>, Value)],
) -> Result<BTreeMap<usize, Value>, String> {
    let mut supplied = BTreeMap::new();
    let mut named = false;
    for (position, (name, value)) in args.iter().enumerate() {
        let index = if let Some(name) = name {
            named = true;
            parameters
                .iter()
                .position(|p| *p == name)
                .ok_or_else(|| format!("unknown argument {name}"))?
        } else {
            if named {
                return Err("positional argument after named argument".into());
            }
            position
        };
        if index >= parameters.len() || supplied.insert(index, value.clone()).is_some() {
            return Err("duplicate or excess argument".into());
        }
    }
    Ok(supplied)
}

/// Whether a checked value is closed ordinary configuration data.
pub fn constant(value: &Value) -> bool {
    if let Kind::Delta(parts) = &value.kind {
        return parts.iter().all(|part| part.operands().all(constant));
    }
    if let Kind::Construct(fields) = &value.kind {
        return fields.iter().all(|(_, v)| constant(v));
    }
    if let Kind::List(values) = &value.kind {
        return values.iter().all(constant);
    }
    matches!(
        value.kind,
        Kind::Prepared(_) | Kind::Literal(_) | Kind::WiringFailure(_)
    )
}

/// Infer generic arguments without resolving dependent sizes or the result type.
pub fn infer_arguments(
    library: &crate::library::Library,
    module: &str,
    signature: &Signature,
    args: &[(Option<String>, Value)],
) -> Result<BTreeMap<String, Ty>, hgl_source::Issue> {
    argument_types(library, module, signature, args, true)
}
fn argument_types(
    library: &crate::library::Library,
    module: &str,
    signature: &Signature,
    args: &[(Option<String>, Value)],
    inference: bool,
) -> Result<BTreeMap<String, Ty>, hgl_source::Issue> {
    let supplied = order_arguments(&signature.parameter_names(), args)?;
    let mut types = BTreeMap::new();
    for (index, value) in supplied {
        let parameter = &signature.parameters[index];
        crate::source_check::argument_requirement(
            library,
            module,
            &parameter.ty,
            &types,
            &value.ty,
        )?;
        let formal = parameter
            .ty
            .strip_prefix("ref<")
            .or_else(|| parameter.ty.strip_prefix("set<"))
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(&parameter.ty);
        if parameter.ty == "signal"
            || ((parameter.constant || signature.value_function)
                && crate::value_types::substitute(
                    library,
                    module,
                    &parameter.ty,
                    &types,
                    &mut BTreeSet::new(),
                )
                .is_ok_and(|ty| crate::value_access::project(&ty) == value.ty))
        {
            continue;
        }
        if signature.generics.iter().any(|name| name == formal)
            && (formal != parameter.ty || matches!(value.ty, Ty::Ref(_)))
        {
            bind_type(signature, parameter, value, &mut types)?;
        } else if signature.generics.iter().any(|name| name == formal)
            || matches!(
                value.ty,
                Ty::Recursive(_)
                    | Ty::Family(_)
                    | Ty::Struct(..)
                    | Ty::List(..)
                    | Ty::Delta(_)
                    | Ty::Map(..)
                    | Ty::Tuple(_)
            )
            || hgl_source::application(&parameter.ty)
                .is_some_and(|(base, _)| matches!(base, "delta" | "atomic" | "rolling"))
        {
            let unify = if inference {
                crate::value_types::infer
            } else {
                crate::value_types::unify
            };
            unify(
                library,
                module,
                if !parameter.constant && !signature.value_function {
                    parameter
                        .ty
                        .strip_prefix("ref<")
                        .and_then(|ty| ty.strip_suffix('>'))
                        .unwrap_or(&parameter.ty)
                } else {
                    &parameter.ty
                },
                if !parameter.constant
                    && !signature.value_function
                    && let Ty::Ref(child) = &value.ty
                {
                    child
                } else {
                    &value.ty
                },
                &signature.generics,
                &mut types,
            )?;
        }
    }
    Ok(types)
}
/// Infer ordinary nested nominal patterns and resolve the selected signature.
pub fn signature_types(
    library: &crate::library::Library,
    module: &str,
    signature: &Signature,
    args: &[(Option<String>, Value)],
) -> Result<BTreeMap<String, Ty>, hgl_source::Issue> {
    let mut types = argument_types(library, module, signature, args, false)?;
    for name in signature
        .parameters
        .iter()
        .map(|p| &p.ty)
        .chain(std::iter::once(&signature.result))
    {
        if matches!(name.as_str(), "signal" | "void") {
            continue;
        }
        if let Some(ty) = resolve_type(name, &types).map_or_else(
            || crate::source_check::bound_type(library, module, name, &types),
            |ty| Ok(Some(ty)),
        )? {
            types.insert(name.clone(), ty);
        }
    }
    Ok(types)
}

pub use crate::native_contracts::bind as native;

/// Match an explicit materialization against exact resolved source types.
pub fn instantiated(
    library: &crate::library::Library,
    module: &str,
    name: &str,
    generics: &[String],
    inferred: &BTreeMap<String, Ty>,
) -> bool {
    library.instances.iter().any(|(owner, item, args)| {
        owner == module
            && item == name
            && args.len() == generics.len()
            && args.iter().zip(generics).all(|(argument, generic)| {
                inferred.get(generic).is_some_and(|actual| {
                    argument == "_"
                        || crate::value_types::resolve(
                            library,
                            module,
                            argument,
                            &mut BTreeSet::new(),
                        )
                        .as_ref()
                            == Ok(actual)
                })
            })
    })
}

/// Bind checked run-preparation expressions without evaluating them.
pub fn bind_prepared(
    signature: &Signature,
    args: &[(Option<String>, Value)],
    runtime: bool,
    hint: Option<&Ty>,
    types: &BTreeMap<String, Ty>,
) -> Result<Bound, String> {
    let mut args = args.to_vec();
    let supplied = order_arguments(&signature.parameter_names(), &args)?;
    for (index, parameter) in signature.parameters.iter().enumerate() {
        if !supplied.contains_key(&index)
            && let Some(Expr::TemporalLiteral(value)) = parameter.default.as_ref().map(Expr::syntax)
        {
            args.push((
                Some(parameter.name.clone()),
                Value::new(value.ty(), Kind::TemporalLiteral(value.clone())),
            ));
        }
    }
    let checked = args
        .iter()
        .enumerate()
        .map(|(id, (name, value))| {
            let value = if matches!(value.kind, Kind::Wire(_)) {
                value.clone()
            } else {
                Value::new(value.ty.clone(), Kind::Prepared(id))
            };
            (name.clone(), value)
        })
        .collect::<Vec<_>>();
    let (mut values, types, result) = bind(signature, &checked, runtime, hint, types)?;
    for value in &mut values {
        if let Kind::Prepared(id) = value.kind {
            *value = args[id].1.clone();
        }
    }
    Ok((values, types, result))
}

/// Preserve supplied argument order and map declaration parameters to local slots.
pub fn ordered_call(
    signature: &Signature,
    values: Vec<Value>,
    supplied: &[(Option<String>, Value)],
) -> Result<(Vec<Value>, Vec<usize>), String> {
    let mut order = supplied
        .iter()
        .enumerate()
        .map(|(index, (label, _))| {
            label
                .as_ref()
                .map_or(Some(index), |name| {
                    signature.parameters.iter().position(|p| p.name == *name)
                })
                .ok_or_else(|| "unknown argument".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    for index in 0..values.len() {
        if !order.contains(&index) {
            order.push(index);
        }
    }
    let mut mapping = vec![0; values.len()];
    let mut values = values.into_iter().map(Some).collect::<Vec<_>>();
    let mut ordered = Vec::new();
    for (slot, parameter) in order.into_iter().enumerate() {
        mapping[parameter] = slot;
        ordered.push(values[parameter].take().ok_or("duplicate argument")?);
    }
    Ok((ordered, mapping))
}
