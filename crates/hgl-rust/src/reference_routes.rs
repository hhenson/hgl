//! Finite checked designation targets reserve their subscriptions before evaluation.
use crate::ir::{Kind, Plan, Statement};
use hgl_source::Ty;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

fn targets(
    plan: &Plan,
    source: usize,
    path: Vec<usize>,
    input_path: Vec<usize>,
    result: &mut BTreeSet<(usize, Vec<usize>, Vec<usize>)>,
) {
    let node = &plan.nodes[source];
    if crate::graph_assemblies::shape(node).is_some() {
        if let Some((&index, rest)) = path.split_first() {
            targets(
                plan,
                node.inputs[index].1,
                rest.to_vec(),
                input_path,
                result,
            );
        } else {
            for (index, (_, source, _)) in node.inputs.iter().enumerate() {
                let mut child_path = input_path.clone();
                child_path.push(index);
                targets(plan, *source, Vec::new(), child_path, result);
            }
        }
        return;
    }
    if !matches!(node.result, Ty::Ref(_)) {
        result.insert((source, path, input_path));
        return;
    }
    if let [(Some(_), body)] = node.handlers.as_slice()
        && let [Statement::Return(value)] = body.as_slice()
        && let Kind::Query(op, args) = &value.kind
        && op == "\0reference"
        && let Kind::Field(parent, index) = &args[0].kind
        && let Kind::Input(input, false) = parent.kind
    {
        let mut child = vec![*index];
        child.extend(path);
        targets(plan, node.inputs[input].1, child, input_path, result);
        return;
    }
    for (_, source, ty) in &node.inputs {
        if matches!(ty, Ty::Ref(_)) {
            targets(plan, *source, path.clone(), input_path.clone(), result);
        }
    }
}

pub(crate) fn reserve(plan: &Plan) -> String {
    let mut subscriptions = BTreeMap::<(usize, Vec<usize>), usize>::new();
    let mut views = Vec::new();
    for (consumer, node) in plan.nodes.iter().enumerate() {
        if crate::graph_assemblies::shape(node).is_some() {
            continue;
        }
        for (input, (_, source, ty)) in node.inputs.iter().enumerate() {
            if matches!(ty, Ty::Ref(_)) || !matches!(plan.nodes[*source].result, Ty::Ref(_)) {
                continue;
            }
            let mut possible = BTreeSet::new();
            targets(plan, *source, Vec::new(), Vec::new(), &mut possible);
            for (source, path, input_path) in possible {
                let target = (source, path);
                *subscriptions.entry(target.clone()).or_default() += 1;
                views.push((consumer, input, input_path, target));
            }
        }
    }
    let mut code = String::new();
    for ((source, path), subscribers) in subscriptions {
        let output = endpoint(source, &path);
        writeln!(
            code,
            "{{let output={output};store.reserve_reference_target(output,{subscribers});}}"
        )
        .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    for (consumer, input, input_path, (source, path)) in views {
        let output = endpoint(source, &path);
        let input = input_path.iter().fold(
            format!("built.inputs[{consumer}][{input}]"),
            |input, index| format!("store.bindings().fixed_input({input},{index})"),
        );
        writeln!(code,"{{let target={output};store.prepare_reference_input({input},target).map_err(|e|format!(\"{{e:?}}\"))?;}}")
            .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    code
}

fn endpoint(source: usize, path: &[usize]) -> String {
    path.iter().fold(
        format!("built.outputs[{source}].expect(\"checked reference target\")"),
        |output, index| format!("store.bindings().fixed_output({output},{index})"),
    )
}
