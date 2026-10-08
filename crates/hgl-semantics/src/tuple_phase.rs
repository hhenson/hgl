//! Source phase checks for tuples at constant boundaries, before specialization.
use crate::library::{Library, Signature};
use hgl_source::{Expr, Issue};
use std::collections::BTreeSet;
/// Whether an expression depends on a known temporal binding.
pub fn depends(expr: &Expr, temporal: &BTreeSet<String>) -> bool {
    match expr.syntax() {
        Expr::Name(name) => temporal.contains(name),
        Expr::Unary(_, v) | Expr::Property(v, _) | Expr::Lambda(_, _, v) => depends(v, temporal),
        Expr::Index(a, b) | Expr::Binary(_, a, b) => depends(a, temporal) || depends(b, temporal),
        Expr::Call(_, args) | Expr::Applied(_, args) => {
            args.iter().any(|(_, v)| depends(v, temporal))
        }
        Expr::Sequence(values) | Expr::Tuple(values) => {
            values.iter().flatten().any(|v| depends(v, temporal))
        }
        Expr::Sparse(values) => values
            .iter()
            .any(|(a, b)| depends(a, temporal) || depends(b, temporal)),
        Expr::Null | Expr::Literal(_) | Expr::TemporalLiteral(_) => false,
        Expr::Located(..) => unreachable!("syntax strips origins"),
    }
}
/// Reject a tuple's temporal children in an explicitly constant context.
pub fn constant(expr: &Expr, temporal: &BTreeSet<String>, tuple: bool) -> Result<(), Issue> {
    if tuple && depends(expr, temporal) {
        return Err(Issue::typed(
            expr.span(),
            "ordinary tuple constant context cannot read a temporal binding",
        ));
    }
    let children: Vec<&Expr> = if let Expr::Tuple(values) | Expr::Sequence(values) = expr.syntax() {
        values.iter().flatten().collect()
    } else if let Expr::Call(_, args) | Expr::Applied(_, args) = expr.syntax() {
        args.iter().map(|(_, v)| v).collect()
    } else {
        Vec::new()
    };
    for child in children {
        constant(child, temporal, matches!(expr.syntax(), Expr::Tuple(_)))?;
    }
    Ok(())
}
/// Check a visible call only when every overload requires the tuple argument constant.
pub fn call(
    library: &Library,
    module: &str,
    test_scope: Option<&str>,
    name: &str,
    args: &[(Option<String>, Expr)],
    scope: &crate::name_check::Scope,
) -> Result<(), Issue> {
    let temporal = &scope.tuple.runtime;
    let (owner, item) = crate::struct_names::identity(library, module, name);
    let signatures: Vec<Signature> = library
        .declarations
        .iter()
        .filter(|d| crate::name_check::visible(library, d, &owner, &item, test_scope))
        .filter_map(|d| d.signature().ok())
        .collect();
    if signatures.is_empty() {
        return Ok(());
    }
    for (position, (label, value)) in args.iter().enumerate() {
        let parameters: Vec<_> = signatures
            .iter()
            .map(|signature| {
                label.as_ref().map_or_else(
                    || signature.parameters.get(position),
                    |label| signature.parameters.iter().find(|p| &p.name == label),
                )
            })
            .collect();
        if parameters.iter().all(|p| p.is_some_and(|p| p.constant)) {
            constant(
                value,
                temporal,
                parameters
                    .iter()
                    .all(|p| p.is_some_and(|p| p.ty.contains("tuple<"))),
            )?;
        }
    }
    Ok(())
}

/// Constant defaults retain the same temporal provenance as their declaration.
pub fn signature(signature: &Signature) -> Result<BTreeSet<String>, Issue> {
    let temporal = signature
        .parameters
        .iter()
        .filter(|p| !signature.value_function && !p.constant)
        .map(|p| p.name.clone())
        .chain((!signature.parameters.iter().any(|p| p.name == "clock")).then_some("clock".into()))
        .collect();
    for parameter in &signature.parameters {
        if let Some(default) = &parameter.default {
            constant(default, &temporal, parameter.ty.contains("tuple<"))?;
        }
    }
    Ok(temporal)
}
