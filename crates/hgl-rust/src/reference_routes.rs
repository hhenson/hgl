//! Finite checked designation targets reserve their subscriptions before evaluation.
use crate::ir::Plan;
use hgl_source::Ty;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

fn targets(
    plan: &Plan,
    source: usize,
    path: Vec<usize>,
    input_path: Vec<usize>,
    result: &mut BTreeSet<(usize, Vec<usize>, Vec<usize>)>,
) -> Result<(), String> {
    let node = &plan.nodes[source];
    if let Some(owner) = plan.conditionals.iter().find(|c| c.owner == source) {
        for branch in &owner.branches {
            targets(
                plan,
                branch.output,
                path.clone(),
                input_path.clone(),
                result,
            )?;
        }
        return Ok(());
    }
    if crate::graph_assemblies::shape(node).is_some() {
        if let Some((&index, rest)) = path.split_first() {
            targets(
                plan,
                node.inputs[index].1,
                rest.to_vec(),
                input_path,
                result,
            )?;
        } else {
            for (index, (_, source, _)) in node.inputs.iter().enumerate() {
                let mut child_path = input_path.clone();
                child_path.push(index);
                targets(plan, *source, Vec::new(), child_path, result)?;
            }
        }
        return Ok(());
    }
    if !matches!(node.result, Ty::Ref(_)) {
        result.insert((source, path, input_path));
        return Ok(());
    }
    for (input, mut child) in crate::reference_designations::inputs(node)? {
        child.extend_from_slice(&path);
        targets(
            plan,
            node.inputs[input].1,
            child,
            input_path.clone(),
            result,
        )?;
    }
    Ok(())
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
            if let Err(issue) = targets(plan, *source, Vec::new(), Vec::new(), &mut possible) {
                return format!("compile_error!({issue:?});");
            }
            for (source, path, input_path) in possible {
                let target = (source, path);
                *subscriptions.entry(target.clone()).or_default() += 1;
                views.push((consumer, input, input_path, target));
            }
        }
    }
    let mut code = String::new();
    if !plan.conditionals.is_empty() {
        for ((source, path), _) in subscriptions {
            let count = views
                .iter()
                .filter(|(_, _, _, target)| target == &(source, path.clone()))
                .map(|(consumer, _, _, _)| format!("conditional_endpoints[{consumer}].len()"))
                .collect::<Vec<_>>()
                .join("+");
            let target = path
                .iter()
                .fold("output.expect(\"checked target\")".into(), |o, index| {
                    format!("store.bindings().fixed_output({o},{index})")
                });
            writeln!(code,"for (_,output) in &conditional_endpoints[{source}] {{store.reserve_reference_target({target},{count});}}").unwrap_or_else(|_|unreachable!("String formatting"));
        }
        for (consumer, input, input_path, (source, path)) in views {
            let target = path
                .iter()
                .fold("output.expect(\"checked target\")".into(), |o, index| {
                    format!("store.bindings().fixed_output({o},{index})")
                });
            let input = input_path
                .iter()
                .fold(format!("inputs[{input}]"), |i, index| {
                    format!("store.bindings().fixed_input({i},{index})")
                });
            writeln!(code,"for (inputs,_) in &conditional_endpoints[{consumer}] {{for (_,output) in &conditional_endpoints[{source}] {{if let Err(e)=store.prepare_reference_input({input},{target}) {{if !matches!(e,hgl_store::BindError::BackwardReference) {{return Err(format!(\"{{e:?}}\").into());}}}}}}}}").unwrap_or_else(|_|unreachable!("String formatting"));
        }
        return code;
    }
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
