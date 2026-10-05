//! Bind checked values to concrete ordinary and temporal signatures.
use hgl_library::{Role, Signature};
use hgl_rust_ir::{Kind, Value};
use hgl_source::{Cursor, Expr, Ty};
use std::collections::{BTreeMap, BTreeSet};
type Bound = (Vec<Value>, BTreeMap<String, Ty>, Ty);
type Arguments = Vec<(Option<String>, Value)>;
/// Check bind at the typed call boundary.
pub fn bind(
    signature: &Signature,
    args: &[(Option<String>, Value)],
    runtime: bool,
    hint: Option<&Ty>,
    declared: &BTreeMap<String, Ty>,
) -> Result<Bound, String> {
    let mut supplied = order_arguments(&parameter_names(signature), args)?;
    let mut types = declared.clone();
    let mut values = Vec::new();
    let mut parameter_names = BTreeSet::new();
    for (index, p) in signature.parameters.iter().enumerate() {
        if !parameter_names.insert(&p.name) {
            return Err("duplicate parameter".into());
        }
        let value = supplied
            .remove(&index)
            .or_else(|| {
                p.default.as_ref().and_then(|e| {
                    if let Expr::Literal(l) = e {
                        Some(Value::new(l.ty(), Kind::Literal(l.clone())))
                    } else {
                        None
                    }
                })
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
    if let Some((parameter, domain)) = &signature.type_domain
        && !types
            .get(parameter)
            .is_some_and(|ty| domain.iter().any(|t| Ty::parse(t).as_ref() == Some(ty)))
    {
        return Err(format!("requires {parameter} in {{{}}}", domain.join(", ")));
    }
    let result = resolve_type(&signature.result, &types).ok_or("unresolved result type")?;
    let result = if signature.value_function && !matches!(result, Ty::Atomic(_)) {
        hgl_value_access::project(&result)
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
pub fn supported_type(ty: &Ty) -> Result<(), String> {
    if !ty.publication() && !matches!(ty, Ty::Void | Ty::Ref(_) | Ty::Nullable(_)) {
        return Err("unsupported temporal publication shape".into());
    }
    if let Ty::Ref(child) = ty {
        if **child == Ty::Void {
            return Err("reference requires a temporal type".into());
        }
        return supported_type(child);
    }
    Ok(())
}

fn bind_type(
    signature: &Signature,
    p: &hgl_library::Parameter,
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
                || hgl_value_access::project(expected) != *actual)
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
                    hgl_value_access::project(&ty)
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
            return Some(hgl_value_access::project(&resolve_type(args[0], types)?).atomic());
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
    Ty::parse(name).or_else(|| types.get(name).cloned())
}

/// Check method arguments at the typed call boundary.
pub fn method_arguments(
    name: &str,
    parameters: &[(&str, Ty)],
    args: &Arguments,
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

/// Infer ordinary nested nominal patterns and resolve the selected signature.
pub fn signature_types(
    library: &hgl_library::Library,
    module: &str,
    signature: &Signature,
    args: &[(Option<String>, Value)],
) -> Result<BTreeMap<String, Ty>, String> {
    let supplied = order_arguments(&parameter_names(signature), args)?;
    let mut types = BTreeMap::new();
    for (index, value) in supplied {
        let parameter = &signature.parameters[index];
        let formal = parameter
            .ty
            .strip_prefix("ref<")
            .or_else(|| parameter.ty.strip_prefix("set<"))
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(&parameter.ty);
        if parameter.ty == "signal"
            || ((parameter.constant || signature.value_function)
                && hgl_value_types::substitute(
                    library,
                    module,
                    &parameter.ty,
                    &types,
                    &mut BTreeSet::new(),
                )
                .is_ok_and(|ty| hgl_value_access::project(&ty) == value.ty))
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
            hgl_value_types::unify(
                library,
                module,
                &parameter.ty,
                &value.ty,
                &signature.generics,
                &mut types,
            )?;
        }
    }
    for name in signature
        .parameters
        .iter()
        .map(|p| &p.ty)
        .chain(std::iter::once(&signature.result))
    {
        if matches!(name.as_str(), "signal" | "void") {
            continue;
        }
        if let Some(ty) = resolve_type(name, &types) {
            types.insert(name.clone(), ty);
        } else if let Ok(ty) =
            hgl_value_types::substitute(library, module, name, &types, &mut BTreeSet::new())
        {
            types.insert(name.clone(), ty);
        }
    }
    Ok(types)
}

/// Select and intern a native provider binding without invoking it.
pub fn native(
    library: &hgl_library::Library,
    declaration: &hgl_library::Decl,
    signature: &Signature,
    (values, result): (Vec<Value>, Ty),
    runtime: bool,
    natives: &mut Vec<hgl_rust_ir::Native>,
) -> Result<Value, String> {
    let module = declaration.module.as_str();
    let name = declaration.name.as_str();
    if !runtime {
        return Err("native value calls in composition are not yet supported".into());
    }
    let selected = library
        .declarations
        .iter()
        .filter(|d| d.module == module && d.name == name && d.role == Role::Native)
        .filter_map(|d| d.signature().ok())
        .filter(|s| {
            s.generics.is_empty()
                && s.parameters.len() == signature.parameters.len()
                && s.parameters
                    .iter()
                    .zip(&signature.parameters)
                    .all(|(a, b)| a.name == b.name && a.ty == b.ty && a.constant == b.constant)
                && s.throws == signature.throws
                && s.result == signature.result
                && !s.body.is_empty()
        })
        .collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err(format!(
            "{module}::{name}: expected one selected native implementation"
        ));
    }
    let mut body = Cursor::new(&selected[0].body);
    body.need("{")?;
    body.lines();
    body.need("}")?;
    body.lines();
    if !body.at("") {
        return Err("unsupported native implementation".into());
    }
    let method = format!(
        "{}_{}",
        name,
        values
            .iter()
            .map(|v| v.ty.name())
            .collect::<Vec<_>>()
            .join("_")
    );
    let full_name = format!("{module}::{name}");
    let native = natives
        .iter()
        .position(|n| {
            n.name == full_name && n.args == values.iter().map(|v| v.ty.clone()).collect::<Vec<_>>()
        })
        .unwrap_or(natives.len());
    if native == natives.len() {
        if natives.iter().any(|n| n.method == method) {
            return Err("native Rust binding name collision".into());
        }
        natives.push(hgl_rust_ir::Native {
            name: full_name,
            method,
            throws: signature.throws,
            args: values.iter().map(|v| v.ty.clone()).collect(),
            result: result.clone(),
        });
    }
    Ok(Value::new(result, Kind::Native(native, values)))
}

/// Match an explicit materialization against exact resolved source types.
pub fn instantiated(
    library: &hgl_library::Library,
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
                        || hgl_value_types::resolve(library, module, argument, &mut BTreeSet::new())
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
    let supplied = order_arguments(&parameter_names(signature), &args)?;
    for (index, parameter) in signature.parameters.iter().enumerate() {
        if !supplied.contains_key(&index)
            && let Some(Expr::TemporalLiteral(value)) = &parameter.default
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
            (
                name.clone(),
                if matches!(value.kind, Kind::Wire(_)) {
                    value.clone()
                } else {
                    Value::new(value.ty.clone(), Kind::Prepared(id))
                },
            )
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

fn parameter_names(signature: &Signature) -> Vec<&str> {
    signature
        .parameters
        .iter()
        .map(|p| p.name.as_str())
        .collect()
}
