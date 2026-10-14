//! Outputless keyed composition preserves member and capture endpoint identities.
use hgl_semantics::ir::{Keyed, Plan};
use hgl_source::Ty;
use std::fmt::Write as _;

/// A synthetic member is a boundary source rather than an executable node.
pub fn member(plan: &Plan, id: usize) -> bool {
    plan.keyed.iter().any(|keyed| keyed.member == id)
}

/// Emit a checked outputless child with a keyed member boundary.
pub fn child(
    plan: &Plan,
    keyed: &Keyed,
    members: &[usize],
    graph: &str,
    inputs: impl Fn(usize) -> Vec<(usize, usize, Vec<String>)>,
) -> String {
    let mut boundaries = String::new();
    for (local, &global) in members.iter().enumerate() {
        for (source, input, path) in inputs(global) {
            if members.contains(&source) {
                continue;
            }
            let (capture, source_path) = if source == keyed.member {
                (0, "hgl_describe::Step::Key")
            } else {
                (
                    plan.nodes[keyed.owner]
                        .inputs
                        .iter()
                        .position(|(_, id, _)| *id == source)
                        .unwrap_or_else(|| unreachable!("checked keyed capture")),
                    "",
                )
            };
            write!(boundaries,"hgl_describe::Boundary {{source_input:{capture},source_path:vec![{source_path}],target:hgl_describe::InputPort {{node:{local},input:{input},path:vec![{}]}}}},",path.join(","))
                .unwrap_or_else(|_| unreachable!("String formatting"));
        }
    }
    format!(
        "hgl_describe::ChildDescription {{graph:{graph},inputs:vec![{boundaries}],output:None,keyed:true}}"
    )
}

/// Emit membership ownership using statically prepared exact input handles.
pub fn node(plan: &Plan, id: usize, shape: impl Fn(&Ty) -> String) -> String {
    let node = &plan.nodes[id];
    let inputs = node
        .inputs
        .iter()
        .map(|(name, _, ty)| format!("({name:?},{}),", shape(ty)))
        .collect::<String>();
    let handles = node
        .inputs
        .iter()
        .map(|(name, _, _)| format!("ports.shaped_input({name:?})?,"))
        .collect::<String>();
    format!(
        r#"
#[derive(Debug)] struct Node{id} {{ captures:Vec<hgl_store::InputId>, changes:Vec<i64>, initial:bool, children:hgl_kernel::prepared_keyed::PreparedKeyed<KeyedBoundary> }}
impl hgl_kernel::Node for Node{id} {{
 fn eval(&mut self,ctx:&mut hgl_kernel::Ctx<'_>)->hgl_kernel::NodeResult {{
  let input=self.captures[0];let now=ctx.evaluation_time();
  self.changes.clear();self.changes.extend(ctx.store().bindings().removed_keys(input));
  for i in 0..self.changes.len() {{self.children.remove(self.changes[i],ctx)?;}}
  self.changes.clear();
  if self.initial {{self.changes.extend(ctx.store().bindings().keys(input));self.initial=false;}}
  else {{self.changes.extend(ctx.store().bindings().added_keys(input));}}
  for i in 0..self.changes.len() {{let key=self.changes[i];self.children.insert(key,ctx,|store,boundary|boundary.refresh(store,&self.captures,key,now))?;}}
  for i in 0..ctx.store().bindings().changed_keys(input).len() {{let key=ctx.store().bindings().changed_keys(input)[i];self.children.refresh(key,ctx,|store,boundary|boundary.refresh_member(store,&self.captures,key,now))?;}}
  self.children.evaluate(ctx)
 }}
 fn stop(&mut self,ctx:&mut hgl_kernel::Ctx<'_>)->hgl_kernel::NodeResult {{self.children.stop(ctx)}}
 fn discard_captured_pass(&mut self,ctx:&mut hgl_kernel::Ctx<'_>) {{self.children.discard_captured_pass(ctx)}}
}}
impl hgl_describe::Buildable for Node{id} {{
 fn node_type()->hgl_types::NodeType {{hgl_types::NodeType {{name:"hgl.keyed#{id}",inputs:vec![{inputs}],output:None,active_inputs:Some(vec![0]),valid_inputs:Some(vec![]),uses_scheduler:true,child_graphs:1,..hgl_types::NodeType::default()}}}}
 fn build(ports:&mut hgl_describe::Ports<'_>)->Result<Self,hgl_describe::BuildError> {{Ok(Self {{captures:vec![{handles}],changes:vec![],initial:true,children:hgl_kernel::prepared_keyed::PreparedKeyed::new(vec![])}})}}
}}
"#
    )
}

/// Cold boundary adaptation decisions; activation performs no shape tests.
pub fn support() -> &'static str {
    r#"
#[derive(Debug)] enum KeyedBinding {Sample,Follow,Designation}
#[derive(Debug)] struct KeyedEdge {target:hgl_store::InputId,source:usize,member:bool,binding:KeyedBinding}
#[derive(Debug)] struct KeyedBoundary {members:Vec<KeyedEdge>,captures:Vec<KeyedEdge>}
struct KeyedQuiet;
impl hgl_store::Wake for KeyedQuiet {fn wake(&mut self,_:hgl_types::NodeId){}}
impl KeyedBoundary {
 fn refresh(&self,store:&mut hgl_store::Store,owners:&[hgl_store::InputId],key:i64,now:hgl_types::EngineTime)->hgl_kernel::NodeResult {
  for edge in self.members.iter().chain(&self.captures) {self.bind(edge,store,owners,key,now)?;} Ok(())
 }
 fn refresh_member(&self,store:&mut hgl_store::Store,owners:&[hgl_store::InputId],key:i64,now:hgl_types::EngineTime)->hgl_kernel::NodeResult {
  for edge in &self.members {
   let source=store.bindings().input(owners[edge.source]).members.prepared_child(key).ok_or_else(||hgl_kernel::NodeError::new("missing prepared keyed member"))?;
   let source=store.bindings().input(source);let target=store.bindings().input(edge.target);
   let unchanged=match edge.binding {
    KeyedBinding::Sample=>target.designation==source.source.map_or(source.designation,|output|store.reference(output))&&target.reference_source.is_none(),
    KeyedBinding::Follow=>target.reference_source==source.source,
    KeyedBinding::Designation=>source.source.is_some_and(|output|target.designation==store.reference(output)),
   };
   if !unchanged {self.bind(edge,store,owners,key,now)?;}
  } Ok(())
 }
 fn bind(&self,edge:&KeyedEdge,store:&mut hgl_store::Store,owners:&[hgl_store::InputId],key:i64,now:hgl_types::EngineTime)->hgl_kernel::NodeResult {
   let mut source=owners[edge.source];
   if edge.member {source=store.bindings().input(source).members.prepared_child(key).ok_or_else(||hgl_kernel::NodeError::new("missing prepared keyed member"))?;}
   let input=store.bindings().input(source);let output=input.source;let reference=output.map_or(input.designation,|output|store.reference(output));
   store.unbind(edge.target);
   match edge.binding {
    KeyedBinding::Sample=>store.sample(edge.target,reference,now,&mut KeyedQuiet),
    KeyedBinding::Follow=>store.follow(edge.target,output.ok_or_else(||hgl_kernel::NodeError::new("missing keyed REF carrier"))?,now,&mut KeyedQuiet),
    KeyedBinding::Designation=>store.bind_designation(edge.target,output.ok_or_else(||hgl_kernel::NodeError::new("missing keyed designation source"))?),
   }.map_err(|error|hgl_kernel::NodeError::new(format!("{error:?}")))?;
  Ok(())
 }
}
fn keyed_boundary(template:&hgl_describe::ChildDescription,built:&hgl_describe::BuiltGraph,owners:&[hgl_store::InputId],store:&hgl_store::Store)->Result<KeyedBoundary,hgl_describe::BuildError> {
 let mut members=vec![];let mut captures=vec![];
 for edge in &template.inputs {
  let source=&store.bindings().input(owners[edge.source_input]).kind;
  let member=!edge.source_path.is_empty();
  let source=if member {match source {hgl_types::TsType::Dictionary(child)|hgl_types::TsType::KeyedDictionary(_,child)|hgl_types::TsType::Growing(child)=>child.as_ref(),_=>unreachable!("checked keyed member")}} else {source};
  let target=built.input(&edge.target,store)?;let target_kind=&store.bindings().input(target).kind;
  let binding=if matches!(target_kind,hgl_types::TsType::Reference(child) if child.as_ref()==source) {KeyedBinding::Designation}
   else if matches!(source,hgl_types::TsType::Reference(child) if child.as_ref()==target_kind) {KeyedBinding::Follow}
   else {KeyedBinding::Sample};
  let edge=KeyedEdge {target,source:edge.source_input,member,binding};
  if member {members.push(edge);} else {captures.push(edge);}
 } Ok(KeyedBoundary {members,captures})
}
"#
}

/// Emit owner preparation inside the enclosing recursive preparation function.
pub fn preparation(plan: &Plan, recursive_prepare: &str) -> String {
    let mut code = String::new();
    for keyed in &plan.keyed {
        let id = keyed.owner;
        write!(code,r#"
if let Some(local)=description.nodes.iter().position(|node|node.implementation=="hgl.keyed#{id}") {{
 let captures=built.graph.node::<Node{id}>(hgl_types::NodeId(local as u32)).expect("checked keyed owner").captures.clone();
 store.prepare_collection_inputs();
 let keys=store.bindings().input(captures[0]).members.prepared.iter().map(|&(key,_)|key).collect::<Vec<_>>();
 let template=&description.nodes[local].children[0];let mut pools=Vec::with_capacity(keys.len());
 for key in keys {{
  let mut pool=Vec::with_capacity(capacity.arrivals);
  for _ in 0..capacity.arrivals {{
   pool.push(hgl_kernel::nested::PreparedChild::new(store,hgl_types::NodeId(local as u32),|store|{{
    let mut child=hgl_describe::instantiate_complete(&template.graph,registry,store).map_err(|error|hgl_kernel::NodeError::new(format!("{{error:?}}")))?;
    prepare_outputs(capacity,store,&template.graph,&child)?;
    let boundary=keyed_boundary(template,&child,&captures,store).map_err(|error|hgl_kernel::NodeError::new(format!("{{error:?}}")))?;
    boundary.refresh(store,&captures,key,hgl_types::EngineTime::NEVER)?;
    {recursive_prepare}
    Ok((child.graph,boundary))
   }}).map_err(|error|error.message)?);
  }} pools.push((key,pool));
 }}
 let node=built.graph.node_mut::<Node{id}>(hgl_types::NodeId(local as u32)).expect("checked keyed owner");node.changes=Vec::with_capacity(pools.len());node.children=hgl_kernel::prepared_keyed::PreparedKeyed::new(pools);
}}
"#).unwrap_or_else(|_| unreachable!("String formatting"));
    }
    code
}
