use crate::syntax::{Cursor, Expr};
use crate::{emit, index, resolve};

/// Checked source tests. Every assertion holds an independent graph.
#[derive(Debug)]
pub struct Suite(Vec<Case>);
#[derive(Debug)]
struct Case {
    name: String,
    plan: hgl_rust::Plan,
    expected: Option<Vec<Option<hgl_rust::Value>>>,
    ordinary: Option<bool>,
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
        let mut scope = hgl_eval_data::Scope::default();
        while !c.take("}") {
            if !c.at("assert") && !c.at("eval") {
                let statement = hgl_eval_data::statement(&mut c)?;
                scope.apply(&statement, |statement, env, next| {
                    resolve::test_statement(library.clone(), &decl.module, statement, env, next)
                })?;
                c.lines();
                continue;
            }
            let values = scope.values()?;
            let assertion = c.take("assert");
            let expr = c.expr()?;
            if assertion && !eval_assertion(&expr) {
                cases.push(Case {
                    name: name.clone(),
                    plan: hgl_rust::Plan::default(),
                    expected: None,
                    ordinary: Some(resolve::assertion(
                        library.clone(),
                        &decl.module,
                        &expr,
                        &values,
                    )?),
                });
                c.lines();
                continue;
            }
            let call = hgl_eval_data::evaluation(expr, assertion)?;
            let plan = resolve::evaluate(
                library.clone(),
                &decl.module,
                &call.function,
                &call.arguments,
                values.clone(),
            )
            .map_err(|e| format!("{name}: {e}"))?;
            let expected = call
                .expected
                .map(|slots| {
                    let (_, ty) = plan
                        .output
                        .as_ref()
                        .ok_or("outputless eval cannot be compared")?;
                    resolve::expected_values(library.clone(), &decl.module, ty, &slots, values)
                        .map_err(|e| format!("{name}: expected output: {e}"))
                })
                .transpose()?;
            cases.push(Case {
                name: name.clone(),
                plan,
                expected,
                ordinary: None,
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
        if let Some(result) = case.ordinary {
            out.push(format!("mod case{i} {{ pub fn test() -> Result<(), String> {{ if {result} {{ Ok(()) }} else {{ Err(\"ordinary assertion failed\".into()) }} }} }}\n"));
            continue;
        }
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

fn eval_assertion(expr: &Expr) -> bool {
    if let Expr::Binary(_, left, _) = expr {
        return matches!(left.as_ref(), Expr::Call(name, _) if name == "eval");
    }
    false
}
