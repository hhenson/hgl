//! Finite checked designation targets reserve their subscriptions before evaluation.
use crate::ir::Plan;
use hgl_source::Ty;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub(crate) fn target_error(plan: &Plan) -> Option<String> {
    plan.nodes.iter().find(|node| matches!(&node.result, Ty::List(child, _) if matches!(child.as_ref(), Ty::Ref(_))))
        .map(|node| format!("Rust target cannot publish ordinary REF-list output in {}", node.name))
}

fn targets(
    plan: &Plan,
    source: usize,
    path: Vec<usize>,
    input_path: Vec<usize>,
    expected: &Ty,
    result: &mut BTreeSet<(usize, Vec<usize>, Vec<usize>)>,
) -> Result<(), String> {
    let node = &plan.nodes[source];
    if path.is_empty() && matches!(expected, Ty::Ref(_)) && node.result == *expected {
        result.insert((source, path, input_path));
        return Ok(());
    }
    if let Some(owner) = plan.conditionals.iter().find(|c| c.owner == source) {
        for branch in &owner.branches {
            targets(
                plan,
                branch.output,
                path.clone(),
                input_path.clone(),
                expected,
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
                expected,
                result,
            )?;
        } else {
            for (index, (_, source, _)) in node.inputs.iter().enumerate() {
                let mut child_path = input_path.clone();
                child_path.push(index);
                let child = child(expected, index)
                    .ok_or("Rust target cannot prepare reference assembly shape")?;
                targets(plan, *source, Vec::new(), child_path, child, result)?;
            }
        }
        return Ok(());
    }
    if !matches!(node.result, Ty::Ref(_)) || node.result == *expected {
        let selected = path
            .iter()
            .try_fold(&node.result, |ty, index| child(ty, *index));
        if selected.is_some_and(|ty| matches!(ty, Ty::Ref(_)) && ty != expected) {
            return Err(format!(
                "Rust target cannot prepare REF child designation flow in {}",
                node.name
            ));
        }
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
            expected,
            result,
        )?;
    }
    Ok(())
}

fn child(ty: &Ty, index: usize) -> Option<&Ty> {
    if let Ty::List(child, Some(size)) = ty {
        (index < *size).then_some(child.as_ref())
    } else if let Ty::Struct(_, fields, _) = ty {
        fields.get(index).map(|(_, child)| child)
    } else if let Ty::Tuple(children) = ty {
        children.get(index)
    } else {
        None
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
            if let Err(issue) = targets(plan, *source, Vec::new(), Vec::new(), ty, &mut possible) {
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
    if views.is_empty() {
        return code;
    }
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
    let nodes = plan.nodes.iter().enumerate().map(|(index,node)| {
        format!("graph.nodes.iter().position(|node|node.implementation=={:?}).expect(\"checked reference route node\")",format!("{}#{index}",node.name))
    }).collect::<Vec<_>>().join(",");
    writeln!(code, "let reference_nodes=[{nodes}];")
        .unwrap_or_else(|_| unreachable!("String formatting"));
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
            format!("built.inputs[reference_nodes[{consumer}]][{input}]"),
            |input, index| format!("store.bindings().fixed_input({input},{index})"),
        );
        writeln!(code,"{{let target={output};store.prepare_reference_input({input},target).map_err(|e|format!(\"{{e:?}}\"))?;}}")
            .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    code
}

fn endpoint(source: usize, path: &[usize]) -> String {
    path.iter().fold(
        format!("built.outputs[reference_nodes[{source}]].expect(\"checked reference target\")"),
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
    fn fixed_selection_reserves_carriers_and_their_designated_payloads_separately() {
        let mut plan = Plan {
            nodes: vec![
                node("first", Ty::I64, Vec::new(), Vec::new()),
                node("second", Ty::I64, Vec::new(), Vec::new()),
            ],
            ..Plan::default()
        };
        let list = hgl_semantics::graph_assemblies::reference_list(
            vec![
                Value::new(Ty::I64, Kind::Wire(0)),
                Value::new(Ty::I64, Kind::Wire(1)),
            ],
            &mut plan,
        )
        .unwrap();
        let Kind::Wire(assembly) = list.kind else {
            unreachable!()
        };
        let selected = Value::new(
            Ty::Ref(Box::new(Ty::I64)),
            Kind::Index(
                Box::new(Value::new(list.ty.clone(), Kind::Input(0, false))),
                Box::new(Value::new(Ty::I64, Kind::Input(1, false))),
            ),
        );
        let selector = plan.nodes.len();
        plan.nodes.push(node(
            "selector",
            selected.ty.clone(),
            vec![("values".into(), assembly, list.ty.clone())],
            vec![Statement::Return(selected)],
        ));
        let mut targets = std::collections::BTreeSet::new();
        super::targets(&plan, selector, vec![], vec![], &Ty::I64, &mut targets).unwrap();
        assert_eq!(
            targets,
            std::collections::BTreeSet::from([(0, vec![], vec![]), (1, vec![], vec![])])
        );
        targets.clear();
        super::targets(&plan, assembly, vec![], vec![], &list.ty, &mut targets).unwrap();
        assert_eq!(
            targets,
            std::collections::BTreeSet::from([(2, vec![], vec![0]), (3, vec![], vec![1])])
        );
    }

    #[test]
    fn finite_preparation_rejects_unmapped_reference_selection_before_execution() {
        let reference = Ty::Ref(Box::new(Ty::I64));
        let collection = Ty::List(Box::new(reference.clone()), None);
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
