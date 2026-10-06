//! Declaration-wide source admission shared by compilation and rejection fixtures.
use hgl_diagnostics::{Diagnostic, Issue};
use hgl_library::{Decl, Library, Role, Signature};
use hgl_source::{Cursor, Expr, Token, Ty};
use std::collections::BTreeMap;
mod body;

/// Check every declaration, including functions unreachable from selected tests.
pub fn validate_sources(sources: &[(String, String)]) -> Vec<Diagnostic> {
    with_semantics(sources, |_, _| Ok(()))
}
/// Combine declaration admission with the ordinary semantic checker, preserving origins.
pub fn with_semantics(
    sources: &[(String, String)],
    check: impl FnMut(&Library, &Decl) -> Result<(), Issue>,
) -> Vec<Diagnostic> {
    check_sources(sources, false, check)
}
/// Check production dependencies and only the root module's test-scoped declarations.
pub fn with_module_semantics(
    sources: &[(String, String)],
    check: impl FnMut(&Library, &Decl) -> Result<(), Issue>,
) -> Vec<Diagnostic> {
    check_sources(sources, true, check)
}
fn check_sources(
    sources: &[(String, String)],
    module_only: bool,
    mut check: impl FnMut(&Library, &Decl) -> Result<(), Issue>,
) -> Vec<Diagnostic> {
    let library = match hgl_library::load_checked(sources) {
        Ok(library) => library,
        Err(error) => return vec![*error],
    };
    let mut errors: Vec<Diagnostic> = Vec::new();
    for declaration in &library.declarations {
        if module_only
            && declaration.module != library.root
            && (declaration.test_only || declaration.role == Role::Test)
        {
            continue;
        }
        let mut issues = declaration_issues(&library, declaration);
        if issues.is_empty() {
            issues.extend(check(&library, declaration).err().map(|issue| {
                issue.at(declaration.tokens.first().map_or(0..0, |t| t.span.clone()))
            }));
        }
        for issue in issues {
            let origin = issue
                .source
                .clone()
                .unwrap_or_else(|| declaration.source.clone());
            let text = sources
                .iter()
                .find(|(name, _)| *name == origin)
                .map_or("", |(_, text)| text.as_str());
            let diagnostic = Diagnostic::new(&origin, text, issue);
            if diagnostic.issue.code.is_none()
                || !errors.iter().any(|existing| {
                    existing.source == diagnostic.source
                        && existing.issue.code == diagnostic.issue.code
                        && existing.issue.span == diagnostic.issue.span
                })
            {
                errors.push(diagnostic);
            }
        }
    }
    errors
}
/// Reject source errors before graph preparation or test execution.
pub fn ensure_sources(sources: &[(String, String)]) -> Result<(), String> {
    let errors = validate_sources(sources);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"))
    }
}
/// Apply the yield-time type rule to an already resolved operand.
pub fn yield_time(ty: &Ty, span: std::ops::Range<usize>) -> Result<(), Issue> {
    if matches!(ty, Ty::Duration | Ty::DateTime) {
        Ok(())
    } else {
        Err(Issue::coded(
            "type",
            "yield.time_type",
            span,
            "yield time requires datetime or duration",
        ))
    }
}
fn declaration_issues(library: &Library, declaration: &Decl) -> Vec<Issue> {
    let checked = match declaration.role {
        Role::Test => hgl_eval_data::steps_checked(&declaration.tokens).map(|_| ()),
        Role::Function | Role::Implementation | Role::Operator | Role::Native => declaration
            .signature_checked()
            .and_then(|signature| function(library, declaration, &signature)),
        Role::Struct => hgl_name_check::structure(library, declaration),
        Role::Enum => hgl_enums::resolve(library, &declaration.module, &declaration.name)
            .map(|_| ())
            .map_err(Issue::from),
    };
    checked
        .err()
        .map(|issue| issue.at(declaration.tokens.first().map_or(0..0, |t| t.span.clone())))
        .into_iter()
        .collect()
}
fn function(library: &Library, declaration: &Decl, signature: &Signature) -> Result<(), Issue> {
    for parameter in &signature.parameters {
        annotation(&parameter.type_tokens)?;
    }
    annotation(&signature.result_tokens)?;
    let types = signature
        .parameters
        .iter()
        .filter_map(|p| {
            hgl_value_types::resolve_ordinary(library, &declaration.module, &p.ty)
                .ok()
                .map(|ty| (p.name.clone(), ty))
        })
        .collect();
    let mut environment = hgl_name_check::Scope {
        types,
        parameters: signature.generics.iter().cloned().collect(),
        ..Default::default()
    };
    if let Some((name, _, _)) = &signature.requirement {
        environment.requirements.insert(name.clone());
    }
    let mut cursor = Cursor::new(&signature.body);
    let checker = body::Checker {
        library,
        module: &declaration.module,
        test_only: declaration.test_only,
    };
    hgl_name_check::signature(library, declaration, signature, &environment)?;
    if declaration.role == Role::Native {
        return native_body(&signature.body);
    }
    if declaration.role == Role::Operator || signature.body.is_empty() {
        return Ok(());
    }

    if cursor.take("=>") {
        checker.expression(&mut cursor, &environment)?;
    } else {
        checker.block(&mut cursor, &mut environment)?;
    }
    cursor.lines();
    if !cursor.at("") {
        return Err(Issue::from("unexpected function body suffix").at(cursor.span()));
    }
    Ok(())
}
// Provider contents remain opaque; only the HGL signature/body boundary is checked.
fn native_body(tokens: &[Token]) -> Result<(), Issue> {
    let mut cursor = Cursor::new(tokens);
    cursor.lines();
    if cursor.at("") {
        return Ok(());
    }
    cursor.need("{")
}
fn annotation(tokens: &[Token]) -> Result<(), Issue> {
    if tokens.is_empty() {
        return Ok(());
    }
    let spelling = tokens
        .iter()
        .filter(|t| t.text != "\n")
        .map(|t| t.text.as_str())
        .collect::<String>();
    let bounds = hgl_type_sizes::expressions(&spelling);
    for bound in &bounds {
        let tokens = hgl_source::lex(bound)?;
        let expression = Cursor::new(&tokens).expr()?;
        if !closed(&expression) {
            return Ok(());
        }
    }
    hgl_type_sizes::normalize_checked(&spelling, &mut |expr| {
        hgl_type_sizes::literal(expr).map_err(Issue::from)
    })
    .map(|_| ())
    .map_err(|mut issue| {
        let mut offset = 0;
        let primary = tokens.iter().filter(|t| t.text != "\n").find(|token| {
            let includes = issue.span.start < offset + token.text.len();
            offset += token.text.len();
            includes
        });
        issue.span = primary.map_or_else(|| tokens[0].span.clone(), |t| t.span.clone());
        issue
    })
}
fn closed(expression: &Expr) -> bool {
    match expression {
        Expr::Literal(_) => true,
        Expr::Unary(_, value) => closed(value),
        Expr::Binary(_, left, right) => closed(left) && closed(right),
        Expr::Null
        | Expr::Property(..)
        | Expr::Index(..)
        | Expr::TemporalLiteral(_)
        | Expr::Name(_)
        | Expr::Sequence(_)
        | Expr::Sparse(_)
        | Expr::Tuple(_)
        | Expr::Call(..)
        | Expr::Applied(..) => false,
    }
}
fn inferred(
    library: &Library,
    module: &str,
    expression: &Expr,
    environment: &BTreeMap<String, Ty>,
) -> Result<Option<Ty>, String> {
    match expression {
        Expr::Literal(value) => Ok(Some(value.ty())),
        Expr::Name(name) => Ok(environment.get(name).cloned()),
        Expr::Unary(_, value) => inferred(library, module, value, environment),
        Expr::Binary(op, left, right) => {
            let left = inferred(library, module, left, environment)?;
            let right = inferred(library, module, right, environment)?;
            match (left, right) {
                (Some(left), Some(right)) => {
                    Ok(hgl_value_check::binary_type(op, &left, &right).ok())
                }
                _ => Ok(None),
            }
        }
        Expr::Call(name, _) => {
            let target = library
                .imports
                .get(&(module.into(), name.clone()))
                .map_or(name.as_str(), String::as_str);
            let (owner, name) = target.rsplit_once("::").unwrap_or((module, target));
            let results = library
                .declarations
                .iter()
                .filter(|d| {
                    d.module == owner
                        && d.name == name
                        && matches!(d.role, Role::Function | Role::Native | Role::Operator)
                })
                .map(|d| {
                    d.signature().ok().and_then(|s| {
                        hgl_value_types::resolve_ordinary(library, owner, &s.result).ok()
                    })
                })
                .collect::<Vec<_>>();
            let common = results.first().cloned().flatten();
            Ok(if results.iter().all(|ty| *ty == common) {
                common
            } else {
                None
            })
        }
        Expr::Property(parent, property)
            if matches!(parent.as_ref(), Expr::Name(name) if name == "clock" && !environment.contains_key(name))
                && (property == "evaluation_time" || property == "now") =>
        {
            Ok(Some(Ty::DateTime))
        }
        Expr::Property(parent, name) => {
            let Some(parent) = inferred(library, module, parent, environment)? else {
                return Ok(None);
            };
            Ok(parent.structure().ok().and_then(|(_, fields, _)| {
                fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, ty)| ty.clone())
            }))
        }
        Expr::TemporalLiteral(_)
        | Expr::Null
        | Expr::Index(..)
        | Expr::Sequence(_)
        | Expr::Sparse(_)
        | Expr::Tuple(_)
        | Expr::Applied(..) => Ok(None),
    }
}

/// Select a callable whose declaration can be checked without generic or required const arguments.
pub fn concrete_signature(declaration: &Decl) -> Result<Option<Signature>, Issue> {
    if !matches!(
        declaration.role,
        Role::Function | Role::Implementation | Role::Operator | Role::Native
    ) {
        return Ok(None);
    }
    let signature = declaration.signature_checked()?;
    if !signature.generics.is_empty()
        || signature
            .parameters
            .iter()
            .any(|p| p.constant && p.default.is_none())
    {
        Ok(None)
    } else {
        Ok(Some(signature))
    }
}
