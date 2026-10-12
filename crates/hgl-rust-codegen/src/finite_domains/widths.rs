use hgl_semantics::ir::{Kind, Node, Plan, Statement};
use hgl_source::Ty;
use std::fmt::Write as _;
fn sum(parts: Vec<String>) -> String {
    parts.into_iter().fold("0usize".into(), |left, right| {
        format!("({left}).checked_add({right}).ok_or(\"prepared mutation width overflow\")?")
    })
}
fn body(statements: &[Statement], node: &Node) -> String {
    crate::mutation_bounds::mutation_width(statements, |collection| {
        let Kind::Input(input, _) = collection.kind else {
            unreachable!("checked added-element loop")
        };
        format!("width{}", node.inputs[input].1)
    })
    .unwrap_or_else(|| unreachable!("finite mutation proof required"))
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
        if matches!(underlying(ty), Ty::Set(_)) {
            node(plan, *source, base, seen, out);
        }
    }
    if !matches!(underlying(&definition.result), Ty::Set(_)) {
        return;
    }
    let mut width = base(underlying(&definition.result));
    for (_, source, ty) in &definition.inputs {
        if matches!(underlying(ty), Ty::Set(_)) {
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
fn underlying(mut ty: &Ty) -> &Ty {
    while let Ty::Ref(child) = ty {
        ty = child;
    }
    ty
}
/// Emit per-output finite publication widths along dependencies, including nested added loops.
/// Constant integer induction loops have exact finite bounds; other while loops remain outside this proof.
pub fn widths(plan: &Plan, base: impl Fn(&Ty) -> String) -> String {
    let mut out = String::new();
    let mut seen = vec![false; plan.nodes.len()];
    for index in 0..plan.nodes.len() {
        node(plan, index, &base, &mut seen, &mut out);
    }
    out
}
