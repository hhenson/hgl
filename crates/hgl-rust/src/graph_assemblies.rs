//! Cold path edges assemble a passive view of the original child endpoints.
use crate::ir::{Kind, Node, Plan, Statement};
use hgl_source::Ty;
use std::fmt::Write as _;

pub(crate) fn shape(node: &Node) -> Option<&Ty> {
    let [(Some(_), body)] = node.handlers.as_slice() else {
        return None;
    };
    let [Statement::Return(value)] = body.as_slice() else {
        return None;
    };
    let Kind::Query(op, args) = &value.kind else {
        return None;
    };
    if op != "\0assembly" || !args.is_empty() {
        return None;
    }
    let Ty::Ref(shape) = &node.result else {
        unreachable!("checked assembly result")
    };
    Some(shape)
}

pub(crate) fn node(index: usize, shape: &str) -> String {
    format!(
        r#"
#[derive(Debug)] struct Node{index} {{ value:hgl_store::InputId, output:hgl_store::OutputId }}
impl hgl_kernel::Node for Node{index} {{
    fn eval(&mut self, ctx:&mut hgl_kernel::Ctx<'_>)->hgl_kernel::NodeResult {{
        let designation=ctx.store().bindings().input_reference(self.value);
        ctx.set_reference(self.output,designation)?;
        Ok(())
    }}
}}
impl hgl_describe::Buildable for Node{index} {{
    fn node_type()->hgl_types::NodeType {{
        hgl_types::NodeType {{ name:"hgl.assembly#{index}", inputs:vec![("value",{shape})],
            output:Some(hgl_types::TsType::Reference(Box::new({shape}))),
            active_inputs:Some(vec![]), valid_inputs:Some(vec![]), schedule_on_start:true,
            ..hgl_types::NodeType::default() }}
    }}
    fn build(ports:&mut hgl_describe::Ports<'_>)->Result<Self,hgl_describe::BuildError> {{
        Ok(Self {{ value:ports.shaped_input("value")?,output:ports.shaped_output()? }})
    }}
}}
"#
    )
}

pub(crate) fn connections(plan: &Plan, node: &Node, index: usize) -> String {
    let mut code = String::new();
    for (source, path) in connections_list(plan, node) {
        let path = path.join(",");
        writeln!(
            code,
            "builder.connect_path(_wire{source},vec![],_wire{index},\"value\",vec![{path}])?;"
        )
        .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    code
}

pub(crate) fn connections_list(plan: &Plan, node: &Node) -> Vec<(usize, Vec<String>)> {
    let mut edges = Vec::new();
    collect_connections(plan, node, &mut Vec::new(), &mut edges);
    edges
}

fn collect_connections(
    plan: &Plan,
    node: &Node,
    path: &mut Vec<String>,
    edges: &mut Vec<(usize, Vec<String>)>,
) {
    for (index, (name, source, ty)) in node.inputs.iter().enumerate() {
        let step = if matches!(shape(node), Some(Ty::List(_, Some(_)))) {
            format!("hgl_describe::Step::Index({index})")
        } else {
            format!("hgl_describe::Step::Field({name:?}.into())")
        };
        path.push(step);
        if !matches!(ty, Ty::Ref(_)) && shape(&plan.nodes[*source]).is_some() {
            collect_connections(plan, &plan.nodes[*source], path, edges);
        } else {
            edges.push((*source, path.clone()));
        }
        path.pop();
    }
}
