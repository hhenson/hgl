//! Callable binding before deferred type specialization, shared with overload selection.
use crate::library::{Decl, Library, Role};
use hgl_source::{Expr, Issue, Ty};
use std::collections::{BTreeMap, BTreeSet};
/// Lexical facts that do not require concrete generic or configuration values.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Inferred concrete types, when available.
    pub types: BTreeMap<String, Ty>,
    /// Declared generic parameter names.
    pub parameters: BTreeSet<String>,
    /// Native callable requirements supplied at specialization.
    pub requirements: BTreeSet<String>,
    /// Injected service bindings, removed by local shadowing.
    pub services: BTreeSet<String>,
}
/// Whether an ordinary callable declaration is visible in this lexical test scope.
pub fn visible(
    library: &Library,
    decl: &Decl,
    module: &str,
    name: &str,
    test_scope: Option<&str>,
) -> bool {
    decl.module == module
        && decl.name == name
        && !matches!(decl.role, Role::Implementation | Role::Test | Role::Struct)
        && (!decl.test_only || test_scope == Some(module))
        && (decl.test_only
            || test_scope != Some(module)
            || !library.declarations.iter().any(|d| {
                d.test_only && d.module == module && d.name == name && d.role == Role::Function
            }))
}
/// Resolve callable identities in a parsed expression without instantiating argument types.
pub fn expression(
    library: &Library,
    module: &str,
    test_scope: Option<&str>,
    expr: &Expr,
    scope: &Scope,
) -> Result<(), Issue> {
    match expr.syntax() {
        Expr::Located(..) => unreachable!("syntax strips source origins"),
        Expr::Call(name, args) | Expr::Applied(name, args) => {
            let base = hgl_source::application(name).map_or(name.as_str(), |(base, _)| base);
            let receiver = args.first().and_then(|(_, value)| {
                if let Expr::Name(name) = value.syntax()
                    && scope.services.contains(name)
                {
                    Some(name.as_str())
                } else {
                    None
                }
            });
            let (owner, item) = crate::struct_names::identity(library, module, base);
            if matches!(expr.syntax(), Expr::Applied(..)) {
                if library
                    .declarations
                    .iter()
                    .any(|d| visible(library, d, &owner, &item, test_scope))
                {
                    return Err("explicit generic arguments require a struct constructor; generic function calls are not admitted".into());
                }
                type_name(library, module, name, scope, test_scope)?;
            }
            if !scope.parameters.contains(base)
                && !scope.requirements.contains(base)
                && !intrinsic(base, receiver, &owner, &item)
                && Ty::parse(base).is_none()
                && !matches!(
                    base,
                    "atomic" | "ref" | "list" | "set" | "map" | "tuple" | "delta"
                )
                && crate::struct_names::declaration(library, module, base)?.is_none()
                && !library
                    .declarations
                    .iter()
                    .any(|d| visible(library, d, &owner, &item, test_scope))
            {
                return Err(format!("{owner}::{item}: expected one matching declaration, found 0: unknown callable {name}").into());
            }
            for (_, argument) in args {
                expression(library, module, test_scope, argument, scope)?;
            }
        }
        Expr::Property(value, _) | Expr::Unary(_, value) => {
            expression(library, module, test_scope, value, scope)?;
        }
        Expr::Index(a, b) | Expr::Binary(_, a, b) => {
            expression(library, module, test_scope, a, scope)?;
            expression(library, module, test_scope, b, scope)?;
        }
        Expr::Sequence(values) | Expr::Tuple(values) => {
            for value in values.iter().flatten() {
                expression(library, module, test_scope, value, scope)?;
            }
        }
        Expr::Sparse(values) => {
            for (key, value) in values {
                expression(library, module, test_scope, key, scope)?;
                expression(library, module, test_scope, value, scope)?;
            }
        }
        Expr::Name(name) => {
            if let Some((owner, _)) = name.split_once("::")
                && !scope.parameters.contains(owner)
                && crate::enums::member(library, module, name)?.is_none()
            {
                return Err(format!("unknown value {name}").into());
            }
        }
        Expr::Lambda(parameters, _, body) => {
            let mut scope = scope.clone();
            for (name, _) in parameters {
                scope.services.remove(name);
            }
            expression(library, module, test_scope, body, &scope)?;
        }
        Expr::Null | Expr::Literal(_) | Expr::TemporalLiteral(_) => {}
    }
    Ok(())
}
fn intrinsic(name: &str, receiver: Option<&str>, owner: &str, item: &str) -> bool {
    receiver.is_some()
        || "schedule schedule_at key_set keys values added removed insert update remove invalidate clear pop scheduled items delta_value elements len push upsert discard contains valid modified all_valid last_modified passivate activate"
            .split_ascii_whitespace().any(|intrinsic| intrinsic == name)
        || (owner == "hgraph.native" && matches!(item, "bound" | "len"))
}

/// Check default and bounded-shape expressions without choosing configuration values.
pub fn signature(
    library: &Library,
    decl: &Decl,
    signature: &crate::library::Signature,
    scope: &Scope,
) -> Result<(), Issue> {
    let test_scope = decl.test_only.then_some(decl.module.as_str());
    if let Some((_, arguments, result)) = &signature.requirement {
        for name in arguments.iter().chain(std::iter::once(result)) {
            type_name(library, &decl.module, name, scope, test_scope)?;
        }
    }
    for parameter in &signature.parameters {
        if let Some(value) = &parameter.default {
            expression(library, &decl.module, test_scope, value, scope)?;
        }
    }
    for tokens in signature
        .parameters
        .iter()
        .map(|p| &p.type_tokens)
        .chain(std::iter::once(&signature.result_tokens))
    {
        if tokens.is_empty() {
            continue;
        }
        let name = tokens
            .iter()
            .filter(|t| t.text != "\n")
            .map(|t| t.text.as_str())
            .collect::<String>();
        type_name(library, &decl.module, &name, scope, test_scope)?;
    }
    Ok(())
}

/// Resolve signature/field nominal identities without instantiating their generic shapes.
pub fn type_name(
    library: &Library,
    module: &str,
    name: &str,
    scope: &Scope,
    test_scope: Option<&str>,
) -> Result<(), Issue> {
    let name = name.strip_prefix("...").unwrap_or(name);
    if name == "_" {
        return Err(
            "struct arguments require complete concrete types; placeholders are not admitted"
                .into(),
        );
    }
    let (base, args) = hgl_source::application(name).unwrap_or((name, Vec::new()));
    if !scope.parameters.contains(base)
        && Ty::parse(base).is_none()
        && !matches!(
            base,
            "signal" | "atomic" | "ref" | "list" | "set" | "map" | "tuple" | "rolling" | "delta"
        )
        && crate::struct_names::declaration(library, module, base)?.is_none()
        && crate::struct_names::enum_declaration(library, module, base)?.is_none()
    {
        return Err(format!("unresolved ordinary type {base}").into());
    }
    for argument in args.iter().take(if matches!(base, "list" | "rolling") {
        1
    } else {
        args.len()
    }) {
        type_name(library, module, argument, scope, test_scope)?;
    }
    for bound in crate::type_sizes::expressions(name) {
        let tokens = hgl_source::lex(bound)?;
        let expr = hgl_source::Cursor::new(&tokens).expr()?;
        expression(library, module, test_scope, &expr, scope)?;
    }
    Ok(())
}
/// Resolve a struct's generic field and parent names through the ordinary nominal index.
pub fn structure(library: &Library, decl: &Decl) -> Result<(), Issue> {
    let schema = decl.required_struct()?;
    let scope = Scope {
        parameters: schema.generics.into_iter().collect(),
        ..Default::default()
    };
    for (_, value) in &schema.defaults {
        expression(
            library,
            &decl.module,
            decl.test_only.then_some(decl.module.as_str()),
            value,
            &scope,
        )?;
    }
    for ty in schema
        .parent
        .iter()
        .chain(schema.fields.iter().map(|(_, ty)| ty))
    {
        type_name(
            library,
            &decl.module,
            ty,
            &scope,
            decl.test_only.then_some(decl.module.as_str()),
        )?;
    }
    Ok(())
}
