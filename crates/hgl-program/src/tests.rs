use crate::syntax::{Cursor, Expr, Literal};
use crate::{emit, index, resolve};

/// Checked source tests. Every assertion holds an independent graph.
#[derive(Debug)]
pub struct Suite(Vec<Case>);
#[derive(Debug)]
struct Case {
    name: String,
    plan: hgl_rust::Plan,
    expected: Option<Vec<Option<Literal>>>,
}

/// Check named tests and their module-wide helpers against the source library.
pub fn compile_tests(sources: &[(String, String)]) -> Result<Suite, String> {
    let library = index::load(sources)?;
    let mut cases = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for decl in &library.declarations {
        if decl.role != index::Role::Test {
            continue;
        }
        let name = format!("{}::{}", decl.module, decl.name);
        if !names.insert(name.clone()) {
            return Err(format!("duplicate test {name}"));
        }
        let mut c = Cursor::new(&decl.tokens);
        c.need("test")?;
        c.name()?;
        c.need("{")?;
        c.lines();
        while !c.take("}") {
            let assertion = c.take("assert");
            let expr = c.expr()?;
            let (call, mut expected) = if assertion {
                let Expr::Binary(op, call, expected) = expr else {
                    return Err(format!("{name}: expected eval comparison"));
                };
                if op != "==" {
                    return Err("eval assertion requires ==".into());
                }
                let Expr::Sequence(elements) = *expected else {
                    return Err("expected a dense sequence".into());
                };
                let expected = elements
                    .into_iter()
                    .map(|v| match v {
                        None => Ok(None),
                        Some(expr) => expr
                            .fixed()
                            .map(Some)
                            .ok_or("expected fixed sequence value".to_owned()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (*call, Some(expected))
            } else {
                (expr, None)
            };
            let Expr::Call(eval, mut args) = call else {
                return Err("expected eval call".into());
            };
            if eval != "eval" || args.is_empty() {
                return Err("expected eval(function, ...)".into());
            }
            let (label, Expr::Name(function)) = args.remove(0) else {
                return Err("eval requires a named function".into());
            };
            if label.is_some() {
                return Err("eval function must be positional".into());
            }
            let plan = resolve::evaluate(library.clone(), &decl.module, &function, &args)
                .map_err(|e| format!("{name}: {e}"))?;
            check_expected(&name, &plan, &mut expected)?;
            cases.push(Case {
                name: name.clone(),
                plan,
                expected,
            });
            c.lines();
        }
    }
    if cases.is_empty() {
        return Err("no tests found".into());
    }
    Ok(Suite(cases))
}

/// Emit a standalone Rust test executable, calling the supplied native provider.
pub fn emit_tests(suite: &Suite) -> String {
    let mut out = Vec::<String>::new();
    for (i, case) in suite.0.iter().enumerate() {
        out.push(format!("mod case{i} {{\n{}\n", emit::emit(&case.plan)));
        out.push(emit::emit_test_body(&case.plan, case.expected.as_deref()));
        out.push("}\n".into());
    }
    out.push("fn main() {\nlet mut failed=0;\n".into());
    for (i, case) in suite.0.iter().enumerate() {
        out.push(format!("match case{i}::test() {{ Ok(())=>println!(\"{{}} ... ok\",{:?}), Err(e)=>{{failed+=1; eprintln!(\"{{}}: {{e}}\",{:?});}} }}\n",case.name,case.name));
    }
    out.push(format!(
        "println!(\"{} tests, {} evaluations, {{failed}} failures\");\n",
        suite
            .0
            .iter()
            .map(|c| &c.name)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        suite.0.len()
    ));
    out.push("if failed != 0 { std::process::exit(1); }\n}\n".into());
    out.concat()
}

fn check_expected(
    name: &str,
    plan: &hgl_rust::Plan,
    expected: &mut Option<Vec<Option<Literal>>>,
) -> Result<(), String> {
    if expected.is_some() && plan.output.is_none() {
        return Err("outputless eval cannot be compared".into());
    }
    if let Some((_, ty)) = &plan.output {
        if matches!(
            ty,
            crate::syntax::Ty::Set(_)
                | crate::syntax::Ty::Ref(_)
                | crate::syntax::Ty::Nullable(_)
                | crate::syntax::Ty::Void
        ) {
            return Err("eval currently records scalar outputs".into());
        }
        if let Some(values) = expected {
            resolve::coerce_sequence(values, ty)
                .map_err(|e| format!("{name}: expected output: {e}"))?;
        }
    }
    Ok(())
}
