//! Compiled capture owners retain original normal endpoints and private error storage.
use hgl_semantics::ir::{Capture, Plan};
use hgl_source::Ty;
use std::fmt::Write as _;

/// Emit an outputless child boundary; the normal endpoint is resolved privately.
pub fn child(
    plan: &Plan,
    capture: &Capture,
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
            let source_input = plan.nodes[capture.owner]
                .inputs
                .iter()
                .position(|(_, id, _)| *id == source)
                .unwrap_or_else(|| unreachable!("checked capture boundary"));
            write!(boundaries, "hgl_describe::Boundary {{source_input:{source_input},source_path:vec![],target:hgl_describe::InputPort {{node:{local},input:{input},path:vec![{}]}}}},", path.join(",")).unwrap_or_else(|_| unreachable!("String formatting"));
        }
    }
    format!(
        "hgl_describe::ChildDescription {{graph:{graph},inputs:vec![{boundaries}],output:None,keyed:false}}"
    )
}

/// Emit one reusable child and a passive typed bundle of original endpoints.
pub fn node(plan: &Plan, id: usize, shape: impl Fn(&Ty) -> String) -> String {
    let node = &plan.nodes[id];
    let Ty::Ref(bundle) = &node.result else {
        unreachable!("checked capture result")
    };
    let mut inputs = String::new();
    let mut handles = String::new();
    for (name, _, ty) in &node.inputs {
        write!(inputs, "({name:?},{}),", shape(ty))
            .unwrap_or_else(|_| unreachable!("String formatting"));
        write!(handles, "ports.shaped_input({name:?})?,")
            .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    let mut active = String::new();
    for i in 0..node.inputs.len() {
        write!(active, "{i},").unwrap_or_else(|_| unreachable!("String formatting"));
    }
    let bundle = shape(bundle);
    format!(
        r#"
#[derive(Debug)] struct Node{id} {{ captures:Vec<hgl_store::InputId>,bundle:hgl_store::InputId,output:hgl_store::OutputId,error:Option<hgl_store::shapes::Output<hgl_store::shapes::Atomic<hgl_store::outcomes::Error>>>,child:Option<hgl_kernel::nested::PreparedChild<CaptureBoundary>>,started:bool,normal:Option<hgl_store::InputId>,choices:std::collections::HashMap<hgl_store::Reference,hgl_store::Reference> }}
impl hgl_kernel::Node for Node{id} {{
 fn eval(&mut self,ctx:&mut hgl_kernel::Ctx<'_>)->hgl_kernel::NodeResult {{
  let now=ctx.evaluation_time();let child=self.child.as_mut().expect("prepared capture child");
  if !self.started {{child.start_bound(ctx,|store,boundary|boundary.refresh(store,&self.captures,now,true))?;self.started=true;}}
  else {{child.refresh_bound(ctx,|store,boundary|boundary.refresh(store,&self.captures,now,false))?;}}
  let outcome=child.evaluate_captured(ctx);ctx.schedule_children(child.next_scheduled_time());
  if let Some(error)=outcome? {{ctx.set_atomic(self.error.expect("prepared capture error"),error)?;}}
  let normal=self.normal.map_or(hgl_store::Reference::default(),|input|ctx.store().bindings().input_reference(input));
  let reference=*self.choices.get(&normal).ok_or_else(||hgl_kernel::NodeError::new("capture normal target was not prepared"))?;
  if ctx.store().bindings().reference_value(self.output)!=reference {{ctx.set_reference(self.output,reference)?;}}
  Ok(())
 }}
 fn stop(&mut self,ctx:&mut hgl_kernel::Ctx<'_>)->hgl_kernel::NodeResult {{if let Some(child)=&mut self.child {{child.stop(ctx)}} else {{Ok(())}}}}
 fn discard_captured_pass(&mut self,ctx:&mut hgl_kernel::Ctx<'_>) {{if self.started {{let child=self.child.as_mut().expect("prepared capture child");child.discard_captured_pass(ctx);ctx.schedule_children(child.next_scheduled_time());}}}}
}}
impl hgl_describe::Buildable for Node{id} {{
 fn node_type()->hgl_types::NodeType {{hgl_types::NodeType {{name:"hgl.capture#{id}",inputs:vec![{inputs}("bundle",{bundle})],output:Some(hgl_types::TsType::Reference(Box::new({bundle}))),active_inputs:Some(vec![{active}]),valid_inputs:Some(vec![]),schedule_on_start:true,uses_scheduler:true,child_graphs:1,..hgl_types::NodeType::default()}}}}
 fn build(ports:&mut hgl_describe::Ports<'_>)->Result<Self,hgl_describe::BuildError> {{Ok(Self {{captures:vec![{handles}],bundle:ports.shaped_input("bundle")?,output:ports.shaped_output()?,error:None,child:None,started:false,normal:None,choices:std::collections::HashMap::new()}})}}
}}
"#
    )
}

/// Emit cold-selected boundary adaptation with generation-aware refresh.
pub fn support() -> &'static str {
    r#"
#[derive(Debug)] enum CaptureBinding {Sample,Follow,Designation}
#[derive(Debug)] struct CaptureEdge {target:hgl_store::InputId,source:usize,binding:CaptureBinding}
#[derive(Debug)] struct CaptureBoundary {edges:Vec<CaptureEdge>,normal:Option<hgl_store::OutputId>,targets:Vec<hgl_store::Reference>}
struct CaptureQuiet;
impl hgl_store::Wake for CaptureQuiet {fn wake(&mut self,_:hgl_types::NodeId){}}
impl CaptureBoundary {
 fn refresh(&self,store:&mut hgl_store::Store,owners:&[hgl_store::InputId],now:hgl_types::EngineTime,force:bool)->hgl_kernel::NodeResult {
  for edge in &self.edges {
   let source=store.bindings().input(owners[edge.source]);let output=source.source;let reference=output.map_or(source.designation,|output|store.reference(output));let target=store.bindings().input(edge.target);
   let unchanged=match edge.binding {CaptureBinding::Sample=>target.designation==reference&&target.reference_source.is_none(),CaptureBinding::Follow=>target.reference_source==output,CaptureBinding::Designation=>target.designation==reference};
   if !force&&unchanged {continue;}
   match edge.binding {
    CaptureBinding::Sample=>store.sample(edge.target,reference,now,&mut CaptureQuiet),
    CaptureBinding::Follow=>store.follow(edge.target,output.ok_or_else(||hgl_kernel::NodeError::new("missing capture REF carrier"))?,now,&mut CaptureQuiet),
    CaptureBinding::Designation=>{store.unbind(edge.target);store.bind_designation(edge.target,output.ok_or_else(||hgl_kernel::NodeError::new("missing capture designation"))?)},
   }.map_err(|error|hgl_kernel::NodeError::new(format!("{error:?}")))?;
  } Ok(())
 }
}
fn capture_boundary(template:&hgl_describe::ChildDescription,built:&hgl_describe::BuiltGraph,owners:&[hgl_store::InputId],store:&hgl_store::Store)->Result<CaptureBoundary,hgl_describe::BuildError> {
 let mut edges=vec![];
 for edge in &template.inputs {
  let source=&store.bindings().input(owners[edge.source_input]).kind;let target=built.input(&edge.target,store)?;let target_kind=&store.bindings().input(target).kind;
  let binding=if matches!(target_kind,hgl_types::TsType::Reference(child) if child.as_ref()==source) {CaptureBinding::Designation}
   else if matches!(source,hgl_types::TsType::Reference(child) if child.as_ref()==target_kind) {CaptureBinding::Follow}
   else {CaptureBinding::Sample};
  edges.push(CaptureEdge {target,source:edge.source_input,binding});
 } Ok(CaptureBoundary {edges,normal:None,targets:vec![]})
}
"#
}

/// Emit recursive cold child preparation and private error endpoint assembly.
pub fn preparation(
    plan: &Plan,
    recursive_prepare: &str,
    normal_targets: impl Fn(usize) -> Vec<(usize, Vec<usize>)>,
) -> String {
    let mut code = String::new();
    for capture in &plan.captures {
        let id = capture.owner;
        let n = plan.nodes.len();
        let normal = capture.output.map_or_else(|| "let normal=None;".into(), |output| {
            if capture.nodes.contains(&output) { format!("let normal=Some(endpoints[{output}].last().and_then(|entry|entry.1).ok_or_else(||hgl_kernel::NodeError::new(\"missing prepared capture normal output\"))?);") }
            else { let input=plan.nodes[id].inputs.iter().position(|(_,source,_)|*source==output).unwrap_or_else(||unreachable!("checked normal capture")); format!("let normal=Some(store.bindings().input(captures[{input}]).source.ok_or_else(||hgl_kernel::NodeError::new(\"missing external capture normal output\"))?);") }
        });
        let field = usize::from(capture.output.is_some());
        let external_normal = capture
            .output
            .is_some_and(|output| !capture.nodes.contains(&output));
        let mut routes = String::new();
        for (source, path) in normal_targets(id) {
            let mut steps = String::new();
            for position in path {
                write!(
                    steps,
                    "target=store.bindings().fixed_output(target,{position});"
                )
                .unwrap_or_else(|_| unreachable!("String formatting"));
            }
            if capture.nodes.contains(&source) {
                write!(routes, "for entry in &endpoints[{source}] {{if let Some(mut target)=entry.1 {{let mut scope=store.bindings().output(target).scope;while scope!=store.scope() {{let Some((parent,_))=store.bindings().child_owner(scope) else {{break}};scope=parent;}}if scope!=store.scope() {{continue;}}{steps}normal_targets.push(store.reference(target));}}}}")
                    .unwrap_or_else(|_| unreachable!("String formatting"));
            } else if let Some(input) = plan.nodes[id]
                .inputs
                .iter()
                .position(|(_, id, _)| *id == source)
            {
                write!(routes,"let mut target=store.bindings().input(captures[{input}]).source.ok_or_else(||hgl_kernel::NodeError::new(\"missing external capture target\"))?;{steps}normal_targets.push(store.reference(target));").unwrap_or_else(|_| unreachable!("String formatting"));
            } else {
                write!(routes,"let mut target=endpoints[{source}].last().and_then(|entry|entry.1).ok_or_else(||hgl_kernel::NodeError::new(\"missing prepared capture target\"))?;{steps}normal_targets.push(store.reference(target));").unwrap_or_else(|_| unreachable!("String formatting"));
            }
        }
        write!(code,r#"
if let Some(local)=description.nodes.iter().position(|node|node.implementation=="hgl.capture#{id}") {{
 let owner=hgl_types::NodeId(local as u32);
 let node=built.graph.node::<Node{id}>(owner).expect("checked capture owner");let captures=node.captures.clone();let bundle=node.bundle;
 let error_id=store.add_shaped_output(owner,<hgl_store::shapes::Atomic<hgl_store::outcomes::Error> as hgl_store::shapes::Shape>::shape());
 let error=hgl_store::shapes::Output::<hgl_store::shapes::Atomic<hgl_store::outcomes::Error>>::bind(store.bindings(),error_id).map_err(|error|format!("{{error:?}}"))?;
 endpoints[{n}+{id}].push((vec![],Some(error_id)));
 let template=&description.nodes[local].children[0];
 let child=hgl_kernel::nested::PreparedChild::new(store,owner,|store|{{
  let mut child=hgl_describe::instantiate_complete(&template.graph,registry,store).map_err(|error|hgl_kernel::NodeError::new(format!("{{error:?}}")))?;
  prepare_outputs(capacity,store,&template.graph,&child)?;
  let mut boundary=capture_boundary(template,&child,&captures,store).map_err(|error|hgl_kernel::NodeError::new(format!("{{error:?}}")))?;
  boundary.refresh(store,&captures,hgl_types::EngineTime::NEVER,true)?;
  {recursive_prepare}
  {normal}
  let mut normal_targets=vec![];{routes}boundary.normal=normal;boundary.targets=normal_targets;Ok((child.graph,boundary))
 }}).map_err(|error|error.message)?;
 let normal=child.boundary().normal;
 let failure=store.bindings().fixed_input(bundle,{field});store.prepare_owned_view(failure,error_id).map_err(|error|format!("{{error:?}}"))?;
 let value=normal.map(|normal|{{let input=store.bindings().fixed_input(bundle,0);(input,normal)}});
 if let Some((input,normal))=value {{
  if {external_normal} {{
   if store.bindings().input(input).kind==store.bindings().output(normal).kind {{store.bind(input,normal)}} else {{store.follow(input,normal,hgl_types::EngineTime::NEVER,&mut CaptureQuiet)}}
  }} else {{store.prepare_owned_view(input,normal)}}.map_err(|error|format!("{{error:?}}"))?;
 }}
 let reference=store.assemble_owned_view(bundle).map_err(|error|format!("{{error:?}}"))?;
 let designation=value.map_or(hgl_store::Reference::default(),|(input,_)|store.bindings().input_reference(input));
 let mut choices=std::collections::HashMap::new();choices.insert(designation,reference);
 if value.is_some() {{for &normal in &child.boundary().targets {{
  let choice=store.items_reference(store.bindings().input(bundle).kind.clone(),vec![normal,store.reference(error_id)]).map_err(|error|format!("{{error:?}}"))?;
  choices.insert(normal,choice);
 }}}}
 let node=built.graph.node_mut::<Node{id}>(owner).expect("checked capture owner");node.error=Some(error);node.child=Some(child);node.normal=value.map(|(input,_)|input);node.choices=choices;
}}
"#).unwrap_or_else(|_| unreachable!("String formatting"));
    }
    code
}
