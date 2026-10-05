use hgl_rust_ir::{Kind, Node, Plan, Statement, Value};
use hgl_source::Ty;
use std::fmt::Write as _;
fn sum(parts: Vec<String>) -> String {
    parts.into_iter().fold("0usize".into(), |left, right| {
        format!("({left}).checked_add({right}).ok_or(\"prepared mutation width overflow\")?")
    })
}
fn body(statements: &[Statement], node: &Node) -> String {
    sum(statements
        .iter()
        .map(|statement| match statement {
            Statement::Call(Value {
                kind: Kind::Query(op, _),
                ..
            }) if op == "set_upsert" || op == "set_discard" => "1usize".into(),
            Statement::If(_, yes, no) => format!("({}).max({})", body(yes, node), body(no, node)),
            Statement::For(_, collection, statements) => {
                let Kind::Input(input, _) = collection.kind else {
                    unreachable!("checked added-element loop")
                };
                let source = node.inputs[input].1;
                format!(
                    "width{source}.checked_mul({}).ok_or(\"prepared loop width overflow\")?",
                    body(statements, node)
                )
            }
            Statement::While(_, statements) => body(statements, node),
            Statement::Exit
            | Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..) => "0usize".into(),
        })
        .collect())
}
fn node(
    plan: &Plan,
    index: usize,
    base: &impl Fn(&Ty) -> String,
    seen: &mut [bool],
    out: &mut String,
) {
    if seen[index] {
        return;
    }
    seen[index] = true;
    let definition = &plan.nodes[index];
    for (_, source, ty) in &definition.inputs {
        if matches!(ty, Ty::Set(_)) {
            node(plan, *source, base, seen, out);
        }
    }
    if !matches!(definition.result, Ty::Set(_)) {
        return;
    }
    let mut width = base(&definition.result);
    for (_, source, ty) in &definition.inputs {
        if matches!(ty, Ty::Set(_)) {
            width = format!("({width}).max(width{source})");
        }
    }
    let mutations = sum(std::iter::once(body(&definition.start, definition))
        .chain(
            definition
                .handlers
                .iter()
                .map(|(_, statements)| body(statements, definition)),
        )
        .collect());
    write!(out, "let width{index}=({width}).max({mutations});")
        .unwrap_or_else(|_| unreachable!("String formatting"));
}
/// Emit per-output finite publication widths along dependencies, including nested added loops.
/// Runtime while loops retain their single-traversal estimate and are outside this finite proof.
pub fn widths(plan: &Plan, base: impl Fn(&Ty) -> String) -> String {
    let mut out = String::new();
    let mut seen = vec![false; plan.nodes.len()];
    for index in 0..plan.nodes.len() {
        node(plan, index, &base, &mut seen, &mut out);
    }
    out
}
