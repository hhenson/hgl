//! Checked finite branch scopes become real prepared child graphs.
use crate::ir::{Conditional, Plan};
use std::collections::BTreeSet;
use std::fmt::Write as _;
pub(crate) fn target_error(plan: &Plan) -> Option<String> {
    if plan.conditionals.is_empty() {
        return None;
    }
    let owned: BTreeSet<_> = plan
        .conditionals
        .iter()
        .flat_map(|c| c.branches.iter().flat_map(|b| b.nodes.iter().copied()))
        .collect();
    let unknown = plan.nodes.iter().enumerate().any(|(i, n)| {
        n.generator.is_some() && owned.contains(&i)
            || n.alarm && n.generator.is_none() && !plan.conditionals.iter().any(|c| c.owner == i)
            || owned.contains(&i) && n.caches.iter().any(|value| value.ty().owning_scalar())
    });
    (!crate::finite_domains::prepared(plan) || unknown).then(||"Rust conditional target requires finite prepared source arrivals, with no child generators or scheduler hooks, and no owning cache reset at activation".into())
}
pub(crate) fn direct(plan: &Plan, members: &[usize]) -> Vec<usize> {
    let nested: BTreeSet<_> = plan
        .conditionals
        .iter()
        .filter(|c| members.contains(&c.owner))
        .flat_map(|c| c.branches.iter().flat_map(|b| b.nodes.iter().copied()))
        .collect();
    members
        .iter()
        .copied()
        .filter(|id| !nested.contains(id))
        .collect()
}
fn path(names: &[String]) -> String {
    names.join(",")
}
fn inputs(plan: &Plan, id: usize) -> Vec<(usize, usize, Vec<String>)> {
    let n = &plan.nodes[id];
    if crate::graph_assemblies::shape(n).is_some() {
        crate::graph_assemblies::connections_list(plan, n)
            .into_iter()
            .map(|(s, p)| (s, 0, p))
            .collect()
    } else {
        n.inputs
            .iter()
            .enumerate()
            .map(|(i, (_, s, _))| (*s, i, vec![]))
            .collect()
    }
}
fn graph(plan: &Plan, members: &[usize]) -> String {
    let ids = direct(plan, members);
    let mut nodes = String::new();
    let mut edges = String::new();
    for (local, &global) in ids.iter().enumerate() {
        let n = &plan.nodes[global];
        let children = plan
            .conditionals
            .iter()
            .find(|c| c.owner == global)
            .map_or_else(String::new, |c| children(plan, c));
        let args = crate::cold_wiring::arguments(plan, n, global);
        write!(nodes,"hgl_describe::NodeDescription {{implementation:{:?}.into(),label:{:?}.into(),scalars:({args} as &[(&str,hgl_types::ScalarValue)]).iter().map(|(n,v)|(String::from(*n),v.clone())).collect(),children:vec![{children}]}},",format!("{}#{global}",n.name),format!("{}#{global}",n.name)).unwrap_or_else(|_|unreachable!("String formatting"));
        for (source, input, p) in inputs(plan, global) {
            if let Some(source) = ids.iter().position(|id| *id == source) {
                write!(edges,"hgl_describe::Edge {{source:hgl_describe::OutputPort {{node:{source},path:vec![]}},target:hgl_describe::InputPort {{node:{local},input:{input},path:vec![{}]}}}},",path(&p)).unwrap_or_else(|_|unreachable!("String formatting"));
            }
        }
    }
    format!(
        "hgl_describe::GraphDescription {{label:\"hgl.branch\".into(),nodes:vec![{nodes}],edges:vec![{edges}]}}"
    )
}
fn children(plan: &Plan, c: &Conditional) -> String {
    let mut code = String::new();
    for b in &c.branches {
        let ids = direct(plan, &b.nodes);
        let mut boundaries = String::new();
        for (local, &global) in ids.iter().enumerate() {
            for (source, input, p) in inputs(plan, global) {
                if ids.contains(&source) {
                    continue;
                }
                let capture = plan.nodes[c.owner]
                    .inputs
                    .iter()
                    .position(|(_, id, _)| *id == source)
                    .unwrap_or_else(|| unreachable!("checked capture"));
                write!(boundaries,"hgl_describe::Boundary {{source_input:{capture},source_path:vec![],target:hgl_describe::InputPort {{node:{local},input:{input},path:vec![{}]}}}},",path(&p)).unwrap_or_else(|_|unreachable!("String formatting"));
            }
        }
        let output = ids
            .iter()
            .position(|id| *id == b.output)
            .unwrap_or_else(|| unreachable!("branch output"));
        write!(code,"hgl_describe::ChildDescription {{graph:{},inputs:vec![{boundaries}],output:Some(hgl_describe::OutputPort {{node:{output},path:vec![]}}),keyed:false}},",graph(plan,&b.nodes)).unwrap_or_else(|_|unreachable!("String formatting"));
    }
    code
}
pub(crate) fn main(plan: &Plan) -> String {
    graph(plan, &(0..plan.nodes.len()).collect::<Vec<_>>())
}
pub(crate) fn node(plan: &Plan, id: usize, shape: impl Fn(&hgl_source::Ty) -> String) -> String {
    let n = &plan.nodes[id];
    let mut inputs = String::new();
    for (name, _, ty) in &n.inputs {
        write!(inputs, "({name:?},{}),", shape(ty))
            .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    let mut handles = String::new();
    for (name, _, _) in n.inputs.iter().skip(1) {
        write!(handles, "ports.shaped_input({name:?})?,")
            .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    format!(
        r#"
#[derive(Debug)] struct Node{id} {{ selector:hgl_store::In<bool>, output:hgl_store::OutputId, captures:Vec<hgl_store::InputId>, branches:hgl_kernel::nested::PreparedSwitch<ConditionalBoundary> }}
impl hgl_kernel::Node for Node{id} {{
 fn eval(&mut self,ctx:&mut hgl_kernel::Ctx<'_>)->hgl_kernel::NodeResult {{
  let enabled=ctx.get(self.selector);let now=ctx.evaluation_time();
  if let Some(boundary)=self.branches.evaluate(enabled,ctx,|store,b|b.refresh(store,now))? {{
   let output=boundary.output;
   if ctx.store().bindings().output(output).modified_at!=hgl_types::EngineTime::NEVER {{let target=ctx.store().bindings().reference_value(output);ctx.set_reference(self.output,target)?;}}
  }} Ok(())
 }}
 fn stop(&mut self,ctx:&mut hgl_kernel::Ctx<'_>)->hgl_kernel::NodeResult {{self.branches.stop(ctx)}}
}}
impl hgl_describe::Buildable for Node{id} {{
 fn node_type()->hgl_types::NodeType {{hgl_types::NodeType {{name:"hgl.conditional#{id}",inputs:vec![{inputs}],output:Some({result}),active_inputs:Some(vec![0]),valid_inputs:Some(vec![0]),uses_scheduler:true,child_graphs:2,..hgl_types::NodeType::default()}}}}
 fn build(ports:&mut hgl_describe::Ports<'_>)->Result<Self,hgl_describe::BuildError> {{
  let selector=ports.input::<bool>("selector")?;let captures=vec![selector.id(),{handles}];Ok(Self {{selector,output:ports.shaped_output()?,captures,branches:hgl_kernel::nested::PreparedSwitch::new([vec![],vec![]])}})
 }}
}}
"#,
        result = shape(&n.result)
    )
}

const SUPPORT: &str = r#"
#[derive(Debug)] struct ConditionalBoundary {output:hgl_store::OutputId,samples:Vec<(hgl_store::InputId,hgl_store::Reference)>,follows:Vec<(hgl_store::InputId,hgl_store::OutputId)>}
struct ConditionalQuiet;
impl hgl_store::Wake for ConditionalQuiet {fn wake(&mut self,_:hgl_types::NodeId){}}
impl ConditionalBoundary {
 fn refresh(&self,store:&mut hgl_store::Store,now:hgl_types::EngineTime)->hgl_kernel::NodeResult {
  for &(input,target) in &self.samples {store.unbind(input);store.sample(input,target,now,&mut ConditionalQuiet).map_err(|e|hgl_kernel::NodeError::new(format!("{e:?}")))?;}
  for &(input,source) in &self.follows {store.unbind(input);store.follow(input,source,now,&mut ConditionalQuiet).map_err(|e|hgl_kernel::NodeError::new(format!("{e:?}")))?;}
  Ok(())
 }
}
type ConditionalEndpoints=Vec<Vec<(Vec<hgl_store::InputId>,Option<hgl_store::OutputId>)>>;
fn conditional_boundary(template:&hgl_describe::ChildDescription,built:&hgl_describe::BuiltGraph,owners:&[hgl_store::InputId],store:&hgl_store::Store)->Result<ConditionalBoundary,hgl_describe::BuildError> {
 let mut samples=vec![];let mut follows=vec![];
 for edge in &template.inputs {
  let source=owners[edge.source_input];let target=built.input(&edge.target,store)?;
  let source=store.bindings().input(source);let target_kind=&store.bindings().input(target).kind;
  if matches!(target_kind,hgl_types::TsType::Reference(t) if t.as_ref()==&source.kind) {continue;}
  if matches!(&source.kind,hgl_types::TsType::Reference(t) if t.as_ref()==target_kind) {follows.push((target,source.source.expect("checked captured REF source")));}
  else {let reference=source.source.map_or(source.designation,|o|store.reference(o));samples.push((target,reference));}
 }
 Ok(ConditionalBoundary {output:built.output(template.output.as_ref().expect("checked child output"),store)?,samples,follows})
}
"#;
pub(crate) fn preparation(plan: &Plan) -> String {
    if plan.conditionals.is_empty() {
        return String::new();
    }
    let mut code = String::from(SUPPORT);
    code.push_str("fn prepare_conditionals(registry:&hgl_describe::Registry,store:&mut hgl_store::Store,description:&hgl_describe::GraphDescription,built:&mut hgl_describe::BuiltGraph,capacity:&FiniteCapacity,endpoints:&mut ConditionalEndpoints)->Result<(),String>{");
    for (id, n) in plan.nodes.iter().enumerate() {
        write!(code,"if let Some(local)=description.nodes.iter().position(|n|n.implementation=={:?}) {{endpoints[{id}].push((built.inputs[local].clone(),built.outputs[local]));}}",format!("{}#{id}",n.name)).unwrap_or_else(|_|unreachable!("String formatting"));
    }
    for c in &plan.conditionals {
        let id = c.owner;
        write!(code,r#"if let Some(local)=description.nodes.iter().position(|n|n.implementation=="hgl.conditional#{id}") {{
 let captures=built.graph.node::<Node{id}>(hgl_types::NodeId(local as u32)).expect("checked owner").captures.clone();
 let mut pools=[Vec::new(),Vec::new()];
 for (branch,template) in description.nodes[local].children.iter().enumerate() {{
  pools[branch].reserve(capacity.arrivals);
  for _ in 0..capacity.arrivals {{
   let child=hgl_kernel::nested::PreparedChild::new(store,hgl_types::NodeId(local as u32),|store|{{
    let mut child=hgl_describe::instantiate_child(template,registry,store,&captures,None,hgl_types::EngineTime::NEVER).map_err(|e|hgl_kernel::NodeError::new(format!("{{e:?}}")))?;
    prepare_outputs(capacity,store,&template.graph,&child)?;
    prepare_conditionals(registry,store,&template.graph,&mut child,capacity,endpoints).map_err(hgl_kernel::NodeError::new)?;
    let boundary=conditional_boundary(template,&child,&captures,store).map_err(|e|hgl_kernel::NodeError::new(format!("{{e:?}}")))?;
    Ok((child.graph,boundary))
   }}).map_err(|e|e.message)?;
   pools[branch].push(child);
  }}
 }}
 built.graph.node_mut::<Node{id}>(hgl_types::NodeId(local as u32)).expect("checked owner").branches=hgl_kernel::nested::PreparedSwitch::new(pools);
}}"#).unwrap_or_else(|_|unreachable!("String formatting"));
    }
    code.push_str("Ok(())}\n");
    code
}
pub(crate) fn instantiate(plan: &Plan) -> String {
    if plan.conditionals.is_empty() {
        return String::new();
    }
    format!(
        "let mut conditional_endpoints:ConditionalEndpoints=(0..{}).map(|_|Vec::new()).collect();prepare_conditionals(&registry,&mut store,&graph,&mut built,&capacity,&mut conditional_endpoints)?;",
        plan.nodes.len()
    )
}
