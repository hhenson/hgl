//! Finite checked designation targets reserve their subscriptions before evaluation.
use crate::ir::Plan;
use hgl_source::Ty;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

fn targets(
    plan: &Plan,
    source: usize,
    path: Vec<usize>,
    result: &mut BTreeSet<(usize, Vec<usize>)>,
) -> Result<(), String> {
    let node = &plan.nodes[source];
    if !matches!(node.result, Ty::Ref(_)) {
        result.insert((source, path));
        return Ok(());
    }
    for (input, mut child) in crate::reference_designations::inputs(node)? {
        child.extend_from_slice(&path);
        targets(plan, node.inputs[input].1, child, result)?;
    }
    Ok(())
}

pub(crate) fn reserve(plan: &Plan) -> String {
    let mut subscriptions = BTreeMap::<(usize, Vec<usize>), usize>::new();
    let mut views = Vec::new();
    for (consumer, node) in plan.nodes.iter().enumerate() {
        for (input, (_, source, ty)) in node.inputs.iter().enumerate() {
            if matches!(ty, Ty::Ref(_)) || !matches!(plan.nodes[*source].result, Ty::Ref(_)) {
                continue;
            }
            let mut possible = BTreeSet::new();
            if let Err(issue) = targets(plan, *source, Vec::new(), &mut possible) {
                return format!("compile_error!({issue:?});");
            }
            for target in possible {
                *subscriptions.entry(target.clone()).or_default() += 1;
                views.push((consumer, input, target));
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
    for (consumer, input, (source, path)) in views {
        let output = endpoint(source, &path);
        writeln!(code,"{{let target={output};store.prepare_reference_input(built.inputs[{consumer}][{input}],target).map_err(|e|format!(\"{{e:?}}\"))?;}}")
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

#[cfg(test)]
mod tests {
    use super::reserve;
    use crate::ir::{Kind, Node, Plan, Statement, Value};
    use hgl_source::{Literal, Ty};

    fn node(
        name: &str,
        result: Ty,
        inputs: Vec<(String, usize, Ty)>,
        body: Vec<Statement>,
    ) -> Node {
        Node {
            name: name.into(),
            result,
            inputs,
            handlers: vec![(None, body)],
            alarm: false,
            generator: None,
            start: Vec::new(),
            stop: Vec::new(),
            global_state: false,
            globals: Vec::new(),
            configuration: Vec::new(),
            caches: Vec::new(),
        }
    }

    #[test]
    fn finite_preparation_rejects_unmapped_reference_selection_before_execution() {
        let reference = Ty::Ref(Box::new(Ty::I64));
        let collection = Ty::List(Box::new(reference.clone()), Some(2));
        let selected = Value::new(
            reference.clone(),
            Kind::Index(
                Box::new(Value::new(collection.clone(), Kind::Input(0, false))),
                Box::new(Value::new(Ty::I64, Kind::Literal(Literal::Int(0)))),
            ),
        );
        let plan = Plan {
            nodes: vec![
                node("source", collection.clone(), Vec::new(), Vec::new()),
                node(
                    "select",
                    reference,
                    vec![("values".into(), 0, collection)],
                    vec![Statement::Return(selected)],
                ),
                node(
                    "consume",
                    Ty::I64,
                    vec![("value".into(), 1, Ty::I64)],
                    Vec::new(),
                ),
            ],
            ..Plan::default()
        };
        let code = reserve(&plan);
        assert!(code.starts_with("compile_error!("));
        assert!(code.contains("Rust target cannot prepare returned designation flow in select"));
        assert!(!code.contains("prepare_reference_input"));
    }
}
