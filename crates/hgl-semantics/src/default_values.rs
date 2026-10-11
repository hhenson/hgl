//! Required ordinary defaults retain exact type and capabilities before source execution.
use crate::{
    ir::{Kind, Value},
    library::{Decl, Library, Role, Signature},
    value_eval::{EvalError, Evaluator},
};
use hgl_source::{Expr, Issue, Ty};
use std::collections::{BTreeMap, BTreeSet};
/// Preserve the checking phase and locate its required operation without inspecting messages.
pub fn error(error: EvalError, expr: &Expr) -> Issue {
    if let EvalError::Coded("value.capability", message) = error {
        Issue::coded("type", "value.constant_capability", expr.span(), message)
    } else {
        Issue::from(format!("required constant evaluation: {error}")).at(expr.span())
    }
}
/// Type checking precedes independent constant retention; contextual literals stay cold recipes.
pub fn required(value: Value, expr: &Expr, expected: &Ty) -> Result<Value, Issue> {
    if value.ty != *expected {
        return Err(Issue::typed(expr.span(), "required default type mismatch"));
    }
    if matches!(value.kind, Kind::TemporalLiteral(_)) {
        return Ok(value);
    }
    Evaluator::default()
        .value(&value)
        .map_err(|e| error(e, expr))
}
/// Materialize concrete ordinary parameter defaults through the authoritative checker.
pub fn parameters(
    library: &Library,
    module: &str,
    signature: &mut Signature,
    types: &BTreeMap<String, Ty>,
    mut check: impl FnMut(&str, &Expr, &Ty) -> Result<Value, Issue>,
) -> Result<(), Issue> {
    for parameter in &mut signature.parameters {
        if parameter.value_default.is_some() || !(parameter.constant || signature.value_function) {
            continue;
        }
        let Some(expr) = &parameter.default else {
            continue;
        };
        if dependent(expr, &|name| {
            signature.generics.iter().any(|p| p == name) && !types.contains_key(name)
        }) {
            continue;
        }
        let Ok(ty) = crate::value_types::concrete(library, module, &parameter.ty, types) else {
            continue;
        };
        let value = if let Expr::TemporalLiteral(literal) = expr.syntax() {
            Value::new(literal.ty(), Kind::TemporalLiteral(literal.clone()))
        } else {
            check(module, expr, &ty)?
        };
        parameter.value_default = Some(required(value, expr, &ty)?);
    }
    Ok(())
}
/// Validate closed unused defaults while deferring dependent specializations.
pub fn declaration(
    library: &Library,
    decl: &Decl,
    mut check: impl FnMut(&str, &Expr, &Ty, &BTreeMap<String, Ty>) -> Result<Value, Issue>,
) -> Result<(), Issue> {
    if matches!(
        decl.role,
        Role::Function | Role::Implementation | Role::Operator | Role::Native
    ) {
        return parameters(
            library,
            &decl.module,
            &mut decl.signature_checked()?,
            &BTreeMap::new(),
            |module, expr, ty| check(module, expr, ty, &BTreeMap::new()),
        );
    }
    if decl.role != Role::Struct {
        return Ok(());
    }
    let (schema, patterns) = crate::inheritance::schema(library, decl)?;
    for (index, expr) in &schema.defaults {
        let (module, source, owner) = &schema.default_origins[index];
        let parameters = crate::inheritance::default_parameters(library, decl, owner)?;
        let types = bindings(library, &parameters, &BTreeMap::new())?;
        if matches!(expr.syntax(), Expr::Null)
            || dependent(expr, &|name| {
                parameters.contains_key(name) && !types.contains_key(name)
            })
            || !closed_pattern(&patterns[*index])
        {
            continue;
        }
        let ty = crate::value_types::field_type(
            library,
            &patterns[*index],
            &BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
        check(module, expr, &ty, &types)
            .and_then(|value| required(value, expr, &ty))
            .map_err(|e| e.in_source(source))?;
    }
    Ok(())
}
fn closed_pattern(pattern: &crate::inheritance::Pattern) -> bool {
    resolved_pattern(pattern, &BTreeMap::new())
}
fn resolved_pattern(pattern: &crate::inheritance::Pattern, types: &BTreeMap<String, Ty>) -> bool {
    match pattern {
        crate::inheritance::Pattern::Parameter(name) => types.contains_key(name),
        crate::inheritance::Pattern::Named(_, _, args) => {
            args.iter().all(|arg| resolved_pattern(arg, types))
        }
    }
}
fn dependent(expr: &Expr, parameter: &impl Fn(&str) -> bool) -> bool {
    let local = if let Expr::Call(name, _) | Expr::Applied(name, _) = expr.syntax() {
        hgl_source::application(name).is_some() && type_dependent(name, parameter)
    } else if let Expr::Lambda(args, result, _) = expr.syntax() {
        args.iter().any(|(_, ty)| type_dependent(ty, parameter))
            || type_dependent(result, parameter)
    } else {
        false
    };
    local
        || expr
            .children()
            .iter()
            .any(|child| dependent(child, parameter))
}

fn type_dependent(name: &str, parameter: &impl Fn(&str) -> bool) -> bool {
    parameter(name)
        || hgl_source::application(name).is_some_and(|(base, args)| {
            args.iter()
                .take(if matches!(base, "list" | "rolling") {
                    1
                } else {
                    args.len()
                })
                .any(|arg| type_dependent(arg, parameter))
        })
}
/// Concrete default-owner bindings, retaining unresolved relationships for later checking.
pub fn bindings(
    library: &Library,
    parameters: &BTreeMap<String, crate::inheritance::Pattern>,
    types: &BTreeMap<String, Ty>,
) -> Result<BTreeMap<String, Ty>, String> {
    parameters
        .iter()
        .filter(|(_, pattern)| resolved_pattern(pattern, types))
        .map(|(name, pattern)| {
            Ok((
                name.clone(),
                crate::value_types::field_type(library, pattern, types, &mut BTreeSet::new())?,
            ))
        })
        .collect()
}

/// Retain legacy fixed scalar/contextual defaults for standalone constructor callers.
pub fn literal(library: &Library, module: &str, expr: &Expr) -> Result<Value, String> {
    let value = crate::enums::default(library, module, expr)?;
    let ty = value.ty();
    let kind = match value {
        hgl_source::ParsedLiteral::Value(v) => Kind::Literal(v),
        hgl_source::ParsedLiteral::Contextual(v) => Kind::TemporalLiteral(v),
    };
    Ok(Value::new(ty, kind))
}

/// Resolve existing closed constant arguments/defaults before dependent size checking.
pub fn size_environment(
    signature: &Signature,
    supplied: &BTreeMap<usize, Value>,
    resolve: impl Fn(&Value) -> Value,
) -> BTreeMap<String, Value> {
    signature
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, p)| p.constant)
        .filter_map(|(id, p)| {
            let value = supplied.get(&id).or(p.value_default.as_ref())?;
            Some((p.name.clone(), resolve(value)))
        })
        .collect()
}

/// A retained direct contextual field still requires run preparation.
pub fn contextual(value: &Value) -> bool {
    matches!(&value.kind, Kind::Construct(fields)
        if fields.iter().any(|(_, field)| matches!(field.kind, Kind::TemporalLiteral(_))))
}

/// Whether authoritative default checking can add any retained parameter values.
pub fn pending(signature: &Signature) -> bool {
    signature
        .parameters
        .iter()
        .any(|p| p.default.is_some() && p.value_default.is_none())
}
