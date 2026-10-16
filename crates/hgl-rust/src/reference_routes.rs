//! Finite checked designation targets reserve their subscriptions before evaluation.
use crate::ir::Plan;
use hgl_source::Ty;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

type Route = (usize, Vec<Option<usize>>, Vec<usize>);
type Target = (usize, Vec<Option<usize>>);
type View = (usize, usize, Vec<usize>, Target);

pub(crate) fn target_error(plan: &Plan) -> Option<String> {
    for capture in &plan.captures {
        if let Err(error) = capture_targets(plan, capture.owner) {
            return Some(error);
        }
    }
    plan.nodes.iter().find(|node| matches!(&node.result, Ty::List(child, _) if matches!(child.as_ref(), Ty::Ref(_))))
        .map(|node| format!("Rust target cannot publish ordinary REF-list output in {}", node.name))
}

fn capture_routes(
    plan: &Plan,
    owner: &crate::ir::Capture,
    path: &[Option<usize>],
    input_path: Vec<usize>,
    expected: &Ty,
    result: &mut BTreeSet<Route>,
) -> Result<(), String> {
    if let Some((&Some(index), rest)) = path.split_first() {
        if let Some(output) = owner.output.filter(|_| index == 0) {
            targets(plan, output, rest.to_vec(), input_path, expected, result)?;
        } else {
            result.insert((plan.nodes.len() + owner.owner, rest.to_vec(), input_path));
        }
    } else {
        let fields = if owner.output.is_some() { 2 } else { 1 };
        for index in 0..fields {
            let mut child_path = input_path.clone();
            child_path.push(index);
            let child = child(expected, index)
                .ok_or("Rust target cannot prepare capture connection shape")?;
            targets(
                plan,
                owner.owner,
                vec![Some(index)],
                child_path,
                child,
                result,
            )?;
        }
    }
    Ok(())
}
fn targets(
    plan: &Plan,
    source: usize,
    path: Vec<Option<usize>>,
    input_path: Vec<usize>,
    expected: &Ty,
    result: &mut BTreeSet<Route>,
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
    if let Some(owner) = plan.captures.iter().find(|capture| capture.owner == source) {
        return capture_routes(plan, owner, &path, input_path, expected, result);
    }
    if crate::graph_assemblies::shape(node).is_some() {
        if let Some((&Some(index), rest)) = path.split_first() {
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
        let selected = path.iter().try_fold(&node.result, |ty, index| match index {
            Some(index) => child(ty, *index),
            None => {
                if let Ty::Map(_, child) = ty {
                    Some(child.as_ref())
                } else {
                    None
                }
            }
        });
        if selected.is_some_and(|ty| matches!(ty, Ty::Ref(_)) && ty != expected) {
            return Err(format!(
                "Rust target cannot prepare REF child designation flow in {}",
                node.name
            ));
        }
        result.insert((source, path, input_path));
        return Ok(());
    }
    if node.name == "hgl.map_connection" {
        let mut member = vec![None];
        member.extend(path);
        return targets(plan, node.inputs[0].1, member, input_path, expected, result);
    }
    for (input, child) in crate::reference_designations::inputs(node)? {
        let mut child = child.into_iter().map(Some).collect::<Vec<_>>();
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

pub(crate) fn capture_targets(plan: &Plan, id: usize) -> Result<Vec<(usize, Vec<usize>)>, String> {
    let capture = plan
        .captures
        .iter()
        .find(|capture| capture.owner == id)
        .ok_or("missing checked capture owner")?;
    let Some(source) = capture.output else {
        return Ok(vec![]);
    };
    let mut possible = BTreeSet::new();
    targets(plan, source, vec![], vec![], &capture.value, &mut possible)?;
    possible
        .into_iter()
        .map(|(source, path, _)| {
            let path = path
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or("Rust capture target cannot prepare changing map member generations")?;
            Ok((source, path))
        })
        .collect()
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
    let mut subscriptions = BTreeMap::<Target, usize>::new();
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
    if crate::conditionals::has_children(plan) {
        return reserve_scoped(subscriptions, views);
    }
    let nodes = plan.nodes.iter().enumerate().map(|(index,node)| {
        format!("graph.nodes.iter().position(|node|node.implementation=={:?}).expect(\"checked reference route node\")",format!("{}#{index}",node.name))
    }).collect::<Vec<_>>().join(",");
    writeln!(code, "let reference_nodes=[{nodes}];")
        .unwrap_or_else(|_| unreachable!("String formatting"));
    for ((source, path), subscribers) in subscriptions {
        code += &visit(
            &endpoint(source),
            &path,
            &format!("store.reserve_reference_target(target,{subscribers});"),
        );
    }
    for (consumer, input, input_path, (source, path)) in views {
        let input = input_path.iter().fold(
            format!("built.inputs[reference_nodes[{consumer}]][{input}]"),
            |input, index| format!("store.bindings().fixed_input({input},{index})"),
        );
        code += &visit(
            &endpoint(source),
            &path,
            &format!(
                "store.prepare_reference_input({input},target).map_err(|e|format!(\"{{e:?}}\"))?;"
            ),
        );
    }
    code
}

fn reserve_scoped(subscriptions: BTreeMap<Target, usize>, views: Vec<View>) -> String {
    let mut code = String::new();
    for ((source, path), _) in subscriptions {
        let count = views
            .iter()
            .filter(|(_, _, _, target)| target == &(source, path.clone()))
            .map(|(consumer, _, _, _)| format!("conditional_endpoints[{consumer}].len()"))
            .collect::<Vec<_>>()
            .join("+");
        let reserve = visit(
            "output.expect(\"checked target\")",
            &path,
            &format!("store.reserve_reference_target(target,{count});"),
        );
        writeln!(
            code,
            "for (_,output) in &conditional_endpoints[{source}] {{{reserve}}}"
        )
        .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    for (consumer, input, input_path, (source, path)) in views {
        let input = input_path
            .iter()
            .fold(format!("inputs[{input}]"), |i, index| {
                format!("store.bindings().fixed_input({i},{index})")
            });
        let prepare = visit(
            "output.expect(\"checked target\")",
            &path,
            &format!(
                "if let Err(e)=store.prepare_reference_input({input},target) {{if !matches!(e,hgl_store::BindError::BackwardReference) {{return Err(format!(\"{{e:?}}\").into());}}}}"
            ),
        );
        writeln!(code,"for (inputs,_) in &conditional_endpoints[{consumer}] {{for (_,output) in &conditional_endpoints[{source}] {{{prepare}}}}}").unwrap_or_else(|_|unreachable!("String formatting"));
    }
    code
}

fn endpoint(source: usize) -> String {
    format!("built.outputs[reference_nodes[{source}]].expect(\"checked reference target\")")
}
// None expands finite prepared map members; Some projects an exact fixed child.
fn visit(root: &str, path: &[Option<usize>], body: &str) -> String {
    let mut code = body.to_owned();
    for step in path.iter().rev() {
        code = match step {
            Some(index) => {
                format!("{{let target=store.bindings().fixed_output(target,{index});{code}}}")
            }
            None => format!(
                "for (_,target) in store.bindings().output(target).members.prepared.clone() {{{code}}}"
            ),
        };
    }
    format!("{{let target={root};{code}}}")
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
