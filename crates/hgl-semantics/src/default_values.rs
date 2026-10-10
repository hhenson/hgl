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
    mut check: impl FnMut(&str, &Expr, &Ty) -> Result<Value, Issue>,
) -> Result<(), Issue> {
    for parameter in &mut signature.parameters {
        if !(parameter.constant || signature.value_function) {
            continue;
        }
        let Some(expr) = &parameter.default else {
            continue;
        };
        if dependent(expr, &signature.generics) {
            continue;
        }
        let Ok(ty) = crate::value_types::resolve_ordinary(library, module, &parameter.ty) else {
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
    mut check: impl FnMut(&str, &Expr, &Ty) -> Result<Value, Issue>,
) -> Result<(), Issue> {
    if matches!(
        decl.role,
        Role::Function | Role::Implementation | Role::Operator | Role::Native
    ) {
        return parameters(library, &decl.module, &mut decl.signature_checked()?, check);
    }
    if decl.role != Role::Struct {
        return Ok(());
    }
    let (schema, patterns) = crate::inheritance::schema(library, decl)?;
    for (index, expr) in &schema.defaults {
        if matches!(expr.syntax(), Expr::Null)
            || dependent(expr, &schema.generics)
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
        let (module, source) = &schema.default_origins[index];
        check(module, expr, &ty)
            .and_then(|value| required(value, expr, &ty))
            .map_err(|e| e.in_source(source))?;
    }
    Ok(())
}
fn closed_pattern(pattern: &crate::inheritance::Pattern) -> bool {
    match pattern {
        crate::inheritance::Pattern::Parameter(_) => false,
        crate::inheritance::Pattern::Named(_, _, args) => args.iter().all(closed_pattern),
    }
}
fn dependent(expr: &Expr, parameters: &[String]) -> bool {
    let name = |name: &str| {
        name.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .any(|word| parameters.iter().any(|p| p == word))
    };
    match expr.syntax() {
        Expr::Located(..) => unreachable!("syntax strips origins"),
        Expr::Name(n) => name(n),
        Expr::Call(n, args) | Expr::Applied(n, args) => {
            name(n) || args.iter().any(|(_, e)| dependent(e, parameters))
        }
        Expr::Lambda(_, ty, e) => name(ty) || dependent(e, parameters),
        Expr::Property(e, _) | Expr::Unary(_, e) => dependent(e, parameters),
        Expr::Index(a, b) | Expr::Binary(_, a, b) => {
            dependent(a, parameters) || dependent(b, parameters)
        }
        Expr::Sequence(items) | Expr::Tuple(items) => {
            items.iter().flatten().any(|e| dependent(e, parameters))
        }
        Expr::Sparse(items) => items
            .iter()
            .any(|(a, b)| dependent(a, parameters) || dependent(b, parameters)),
        Expr::Null | Expr::Literal(_) | Expr::TemporalLiteral(_) => false,
    }
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
            let value = supplied
                .get(&id)
                .cloned()
                .or_else(|| p.value_default.clone())
                .or_else(|| {
                    p.default
                        .as_ref()?
                        .fixed()
                        .map(|v| Value::new(v.ty(), Kind::Literal(v)))
                })?;
            Some((p.name.clone(), resolve(&value)))
        })
        .collect()
}
