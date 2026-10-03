use crate::ir::{Kind, Node, Plan, Value};
use hgl_rust_values::{
    condition_code, global_markers, global_schema, global_type, literal, query, rust_type,
    scalar_type, statements,
};
use hgl_source::{Literal, Ty};

fn node(plan: &Plan, n: &Node, index: usize, out: &mut Vec<String>) {
    out.push(format!("#[derive(Debug)]\nstruct Node{index} {{\n"));
    for (i, (_, ty)) in n.globals.iter().enumerate() {
        out.push(format!(
            "global{i}: hgl_store::Global<{}>,\n",
            global_type(ty)
        ));
    }
    if let Some((name, ty)) = &n.capability {
        out.push(format!(
            "{name}: hgl_std_native::eval_buffers::{}<{}>,\n",
            if name == "replay_input" {
                "ReplayInput"
            } else {
                "Capture"
            },
            rust_type(ty)
        ));
    }
    for (i, (_, _, ty)) in n.inputs.iter().enumerate() {
        out.push(if matches!(ty, Ty::Ref(_) | Ty::Set(_)) {
            format!("input{i}: hgl_store::InputId,\n")
        } else {
            format!("input{i}: hgl_store::In<{}>,\n", rust_type(ty))
        });
    }
    for (i, cache) in n.caches.iter().enumerate() {
        out.push(format!("cache{i}: {},\n", rust_type(&cache.ty())));
    }
    if n.result != Ty::Void {
        out.push(if matches!(n.result, Ty::Ref(_) | Ty::Set(_)) {
            "_output: hgl_store::OutputId,\n".into()
        } else {
            format!("_output: hgl_store::Out<{}>,\n", rust_type(&n.result))
        });
    }
    out.push(format!("}}\nimpl hgl_kernel::Node for Node{index} {{\nfn start(&mut self, _ctx: &mut hgl_kernel::Ctx<'_>) -> hgl_kernel::NodeResult {{\n"));
    for (i, cache) in n.caches.iter().enumerate() {
        out.push(format!("self.cache{i} = {};\n", literal(cache)));
    }
    statements(plan, &n.start, out);
    out.push("Ok(())\n}\nfn eval(&mut self, _ctx: &mut hgl_kernel::Ctx<'_>) -> hgl_kernel::NodeResult {\n" .to_owned());
    node_eval(plan, n, out);
    node_build(plan, n, index, out);
}
fn node_eval(plan: &Plan, n: &Node, out: &mut Vec<String>) {
    let guard = if n.inputs.is_empty() {
        n.alarm.to_string()
    } else {
        "true".into()
    };
    out.push(format!("if {guard} {{\n"));
    for (condition, body) in &n.handlers {
        let guard = condition
            .as_ref()
            .map_or_else(|| input_guard(n), |v| condition_code(plan, v));
        out.push(format!("if {guard} {{\n"));
        statements(plan, body, out);
        out.push("}\n".into());
    }
    out.push("}\nOk(())\n}\nfn stop(&mut self, _ctx: &mut hgl_kernel::Ctx<'_>) -> hgl_kernel::NodeResult {\n".into());
    statements(plan, &n.stop, out);
    out.push("Ok(())\n}\n}\n".into());
}
fn node_build(plan: &Plan, n: &Node, index: usize, out: &mut Vec<String>) {
    out.push(format!("impl hgl_describe::Buildable for Node{index} {{\nfn node_type() -> hgl_types::NodeType {{\nhgl_types::NodeType {{ name: {:?}, inputs: vec![",format!("{}#{index}",n.name)));
    for (name, _, ty) in &n.inputs {
        out.push(format!("({name:?},{}),", shape(ty)));
    }
    out.push("],\n".to_owned());
    if n.global_state {
        out.push("uses_global_state: true,\nglobal_entries: vec![".into());
        for (key, ty) in &n.globals {
            out.push(format!("({key:?}, {}),", global_schema(ty)));
        }
        out.push("],\n".into());
    }
    if n.result != Ty::Void {
        out.push(format!("output: Some({}),\n", shape(&n.result)));
    }
    if n.inputs.iter().any(|(_, _, t)| matches!(t, Ty::Ref(_))) {
        out.push("schedule_on_start: true,\n".into());
    }
    if n.handlers.iter().any(|(guard, _)| guard.is_some()) {
        out.push("valid_inputs: Some(vec![]),\n".into());
    }
    out.push(format!("uses_scheduler: {}, ..hgl_types::NodeType::default() }}\n}}\nfn build(ports: &mut hgl_describe::Ports<'_>) -> Result<Self,hgl_describe::BuildError> {{\nOk(Self {{\n",n.alarm));
    for (i, (key, ty)) in n.globals.iter().enumerate() {
        out.push(format!(
            "global{i}: ports.global::<{}>({key:?})?,\n",
            global_type(ty)
        ));
    }
    if let Some((name, _)) = &n.capability {
        let configured = if name == "replay_input" {
            plan.replay_inputs.iter().find(|(id, _)| *id == index).map(|(_, slots)| format!("hgl_std_native::eval_buffers::ReplayInput::new(vec![{}], hgl_types::EngineTime::MIN_START).map_err(|e| hgl_describe::BuildError::InvalidNodeType {{ node: {:?}.into(), what: e.message }})?", sequence(slots), n.name))
        } else {
            plan.output
                .as_ref()
                .filter(|(id, _)| *id == index)
                .map(|_| "hgl_std_native::eval_buffers::Capture::new()".into())
        };
        let configured = configured.unwrap_or_else(|| format!("return Err(hgl_describe::BuildError::InvalidNodeType {{ node: {:?}.into(), what: {:?}.into() }})", n.name, format!("{name}: missing configured binding")));
        out.push(format!("{name}: {configured},\n"));
    }
    for (i, (name, _, ty)) in n.inputs.iter().enumerate() {
        out.push(format!(
            "input{i}: ports.{}({name:?})?,\n",
            if matches!(ty, Ty::Ref(_) | Ty::Set(_)) {
                "shaped_input"
            } else {
                "input"
            }
        ));
    }
    for (i, cache) in n.caches.iter().enumerate() {
        out.push(format!("cache{i}: {},\n", literal(cache)));
    }
    if n.result != Ty::Void {
        out.push(format!(
            "_output: ports.{}()?,\n",
            if matches!(n.result, Ty::Ref(_) | Ty::Set(_)) {
                "shaped_output"
            } else {
                "output"
            }
        ));
    }
    out.push("})\n}\n}\n".to_owned());
}
fn comment(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}
/// Emit runtime nodes, selected native signatures and graph construction.
pub fn emit(plan: &Plan) -> String {
    let mut out = vec![String::from("// Generated from checked HGL source.\n")];
    for doc in &plan.docs {
        for line in doc.lines() {
            out.push(format!("// {}\n", comment(line)));
        }
    }
    if !plan.natives.is_empty() {
        out.push("/// Selected native scalar interfaces.\npub trait Native {\n".to_owned());
    }
    for native in &plan.natives {
        out.push(format!(
            "/// HGL `{}`.\nfn {}({}) -> {};\n",
            native.name,
            native.method,
            native
                .args
                .iter()
                .enumerate()
                .map(|(i, ty)| format!("arg{i}: {}", native_arg_type(ty)))
                .collect::<Vec<_>>()
                .join(", "),
            native_result(native)
        ));
    }
    if !plan.natives.is_empty() {
        out.push("}\n".to_owned());
    }
    out.push(global_markers(plan));
    for (i, n) in plan.nodes.iter().enumerate() {
        node(plan, n, i, &mut out);
    }
    out.push("/// Register the selected HGL implementations.\npub fn register(registry: &mut hgl_describe::Registry) -> Result<(),hgl_describe::BuildError> {\n" .to_owned());
    for i in 0..plan.nodes.len() {
        out.push(format!("registry.register::<Node{i}>()?;\n"));
    }
    out.push("Ok(())\n}\n/// Construct the checked entry graph without executing it.\npub fn main(registry: &hgl_describe::Registry) -> Result<hgl_describe::GraphDescription,hgl_describe::BuildError> {\nlet mut builder = hgl_describe::Builder::new(\"hgl.program\",registry);\n" .to_owned());
    for (i, n) in plan.nodes.iter().enumerate() {
        out.push(format!(
            "let _wire{i} = builder.node({:?}, &[])?;\n",
            format!("{}#{i}", n.name)
        ));
        for (name, source, _) in &n.inputs {
            out.push(format!(
                "builder.connect(_wire{source},_wire{i},{name:?})?;\n"
            ));
        }
    }
    out.push("builder.finish()\n}\n".to_owned());
    out.concat()
}

/// Emit execution and observation of one checked eval plan.
pub fn emit_test_body(plan: &Plan, expected: Option<&[Option<Literal>]>) -> String {
    let mut out = Vec::<String>::new();
    if !plan.natives.is_empty() {
        out.push("impl Native for crate::Provider {\n".into());
    }
    for native in &plan.natives {
        let args = native
            .args
            .iter()
            .enumerate()
            .map(|(i, t)| format!("arg{i}: {}", native_arg_type(t)))
            .collect::<Vec<_>>()
            .join(",");
        let call = (0..native.args.len())
            .map(|i| format!("arg{i}"))
            .collect::<Vec<_>>()
            .join(",");
        out.push(format!(
            "fn {}({args})->{} {{ crate::native::{}({call}) }}\n",
            native.method,
            native_result(native),
            native.method
        ));
    }
    if !plan.natives.is_empty() {
        out.push("}\n".into());
    }
    out.push("pub fn test()->Result<(),String> {\nlet mut registry=hgl_describe::Registry::new();\nregister(&mut registry).map_err(|e|format!(\"{e:?}\"))?;\nlet graph=main(&registry).map_err(|e|format!(\"{e:?}\"))?;\n".into());
    out.push("let mut store=hgl_store::Store::new();\nstore.provision_global_state();\nlet mut built=hgl_describe::instantiate_complete(&graph,&registry,&mut store).map_err(|e|format!(\"{e:?}\"))?;\nhgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig { start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::MAX_END }).map_err(|e|format!(\"{e:?}\"))?;\n".into());
    if let Some((record, _)) = &plan.output {
        let identity = format!("{}#{record}", plan.nodes[*record].name);
        out.push(format!("let record=u32::try_from(graph.nodes.iter().position(|n|n.implementation=={identity:?}).ok_or(\"missing eval record\")?).map_err(|e|e.to_string())?;\nlet ticks=built.graph.node_mut::<Node{record}>(hgl_types::NodeId(record)).ok_or(\"missing eval record\")?.capture.take_ticks().map_err(|e|e.message)?;\ndrop(built);\nlet _observed=hgl_testkit::evaluation::observe(ticks,{})?;\n", plan.input_length));
        if let Some(expected) = expected {
            out.push(format!(
                "hgl_testkit::evaluation::compare(&[{}],&_observed)?;\n",
                sequence(expected)
            ));
        }
    }
    out.push("Ok(())\n}\n".into());
    out.concat()
}

fn native_result(native: &crate::ir::Native) -> String {
    if native.throws {
        format!(
            "Result<{},Box<hgl_kernel::NodeError>>",
            rust_type(&native.result)
        )
    } else {
        rust_type(&native.result).to_owned()
    }
}

fn shape(ty: &Ty) -> String {
    if let Ty::Ref(t) = ty {
        return format!("hgl_types::TsType::Reference(Box::new({}))", shape(t));
    }
    if let Ty::Set(t) = ty {
        return format!(
            "hgl_types::TsType::Set(hgl_types::ScalarType::{})",
            scalar_type(t)
        );
    }
    format!(
        "hgl_types::TsType::Ts(hgl_types::ScalarType::{})",
        scalar_type(ty)
    )
}

fn input_guard(n: &Node) -> String {
    if n.inputs.is_empty() {
        return n.alarm.to_string();
    }
    n.inputs
        .iter()
        .enumerate()
        .map(|(i, (_, _, ty))| query("valid", &[Value::new(ty.clone(), Kind::Input(i, false))]))
        .collect::<Vec<_>>()
        .join(" && ")
}

fn native_arg_type(ty: &Ty) -> &'static str {
    if *ty == Ty::Str {
        "&str"
    } else {
        rust_type(ty)
    }
}
fn sequence(slots: &[Option<Literal>]) -> String {
    slots
        .iter()
        .map(|l| {
            l.as_ref()
                .map_or_else(|| "None".into(), |l| format!("Some({})", literal(l)))
        })
        .collect::<Vec<_>>()
        .join(",")
}
