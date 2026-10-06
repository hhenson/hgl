use crate::ir::{Kind, Node, Plan, Value};
use std::fmt::Write as _;
fn append(out: &mut String, args: std::fmt::Arguments<'_>) {
    out.write_fmt(args)
        .unwrap_or_else(|_| unreachable!("String formatting"));
}
use hgl_rust_generators::Generator;
use hgl_rust_values::{
    condition_code, global_markers, global_schema, global_type, literal, owned_type, query,
    rust_type, scalar_type, statements,
};
use hgl_source::Ty;

fn node(plan: &Plan, n: &Node, index: usize, out: &mut Vec<String>) {
    let generator = n.generator.as_deref().map(Generator::lower);
    out.push(format!("struct Node{index} {{\n"));
    if let Some(generator) = &generator {
        out.push(generator.fields(&n.result));
    }
    for (i, (_, ty)) in n.globals.iter().enumerate() {
        out.push(format!(
            "global{i}: hgl_store::Global<{}>,\n",
            global_type(ty)
        ));
    }
    out.push("configuration_columns:hgl_store::ValueColumns,\n".into());
    for (id, value) in n.configuration.iter().enumerate() {
        out.push(format!(
            "configuration{id}: {},configuration_slot{id}:hgl_store::ValueSlot<{}>,\n",
            owned_type(&value.ty),
            global_type(&value.ty)
        ));
    }
    for (i, (_, _, ty)) in n.inputs.iter().enumerate() {
        out.push(if hgl_rust_deltas::shaped(ty) {
            format!(
                "input{i}: hgl_store::shapes::Input<{}>,\n",
                hgl_rust_deltas::shape_marker(ty)
            )
        } else if matches!(ty, Ty::Ref(_)) {
            format!("input{i}: hgl_store::InputId,\n")
        } else {
            format!("input{i}: hgl_store::In<{}>,\n", rust_type(ty))
        });
    }
    for (i, cache) in n.caches.iter().enumerate() {
        out.push(format!("cache{i}: {},\n", rust_type(&cache.ty())));
    }
    if n.result != Ty::Void {
        out.push(if hgl_rust_deltas::shaped(&n.result) {
            format!(
                "_output: hgl_store::shapes::Output<{}>,\n",
                hgl_rust_deltas::shape_marker(&n.result)
            )
        } else if matches!(n.result, Ty::Ref(_)) {
            "_output: hgl_store::OutputId,\n".into()
        } else {
            format!("_output: hgl_store::Out<{}>,\n", rust_type(&n.result))
        });
    }
    out.push(format!("}}\nimpl std::fmt::Debug for Node{index} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.debug_struct(\"Node{index}\").finish_non_exhaustive() }} }}\nimpl hgl_kernel::Node for Node{index} {{\nfn start(&mut self, _ctx: &mut hgl_kernel::Ctx<'_>) -> hgl_kernel::NodeResult {{\n"));
    for (i, cache) in n.caches.iter().enumerate() {
        out.push(format!("self.cache{i} = {};\n", literal(cache)));
    }
    if let Some(generator) = &generator {
        out.push(generator.start());
    }
    statements(plan, &n.start, out, Some(&n.result));
    out.push("Ok(())\n}\nfn eval(&mut self, _ctx: &mut hgl_kernel::Ctx<'_>) -> hgl_kernel::NodeResult {\n" .to_owned());
    node_eval(plan, n, generator.as_ref(), out);
    node_build(plan, n, index, generator.as_ref(), out);
}
fn node_eval(plan: &Plan, n: &Node, generator: Option<&Generator>, out: &mut Vec<String>) {
    if let Some(generator) = generator {
        out.push(generator.evaluation(plan, &n.result));
        out.push("}\n}\n".into());
        return;
    }
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
        statements(plan, body, out, Some(&n.result));
        out.push("}\n".into());
    }
    out.push("}\nOk(())\n}\nfn stop(&mut self, _ctx: &mut hgl_kernel::Ctx<'_>) -> hgl_kernel::NodeResult {\n".into());
    statements(plan, &n.stop, out, Some(&n.result));
    out.push("Ok(())\n}\n}\n".into());
}
fn node_build(
    plan: &Plan,
    n: &Node,
    index: usize,
    generator: Option<&Generator>,
    out: &mut Vec<String>,
) {
    node_type(n, index, out);
    build_entry(plan, n, index, out);
    out.push(hgl_rust_keyed::node_preparation(n, |v| {
        condition_code(plan, v)
    }));
    out.push(hgl_rust_source_slots::initialize(
        &n.configuration,
        true,
        &n.name,
        |value| condition_code(plan, value),
    ));
    out.push("Ok(Self {configuration_columns,\n".into());
    if let Some(generator) = generator {
        out.push(generator.initialize());
    }
    for (i, (key, ty)) in n.globals.iter().enumerate() {
        out.push(format!(
            "global{i}: ports.global::<{}>({key:?})?,\n",
            global_type(ty)
        ));
    }
    for id in 0..n.configuration.len() {
        out.push(format!("configuration{id},configuration_slot{id},\n"));
    }
    for (i, (name, _, ty)) in n.inputs.iter().enumerate() {
        if hgl_rust_deltas::shaped(ty) {
            out.push(format!("input{i}: {{let id=ports.shaped_input({name:?})?; hgl_store::shapes::Input::<{}>::bind(ports.store().bindings(),id).map_err(hgl_describe::BuildError::Bind)?}},\n",hgl_rust_deltas::shape_marker(ty)));
            continue;
        }
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
    if hgl_rust_deltas::shaped(&n.result) {
        out.push(format!("_output: {{let id=ports.shaped_output()?; hgl_store::shapes::Output::<{}>::bind(ports.store().bindings(),id).map_err(hgl_describe::BuildError::Bind)?}},\n",hgl_rust_deltas::shape_marker(&n.result)));
    } else if n.result != Ty::Void {
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
fn build_entry(plan: &Plan, node: &Node, index: usize, out: &mut Vec<String>) {
    if node.configuration.is_empty() {
        out.push(
            "fn build(ports:&mut hgl_describe::Ports<'_>)->Result<Self,hgl_describe::BuildError> {"
                .into(),
        );
        return;
    }
    let body = if node_prepared(node) {
        format!(
            "Err(hgl_describe::BuildError::InvalidNodeType {{node:{:?},what:\"prepared configuration required\".into()}})",
            node.name
        )
    } else {
        let args=node.configuration.iter().map(|value|format!("(||->hgl_types::NodeResult<_>{{Ok({})}})().map_err(|e|hgl_describe::BuildError::InvalidNodeType {{node:{:?},what:e.message}})?",condition_code(plan,value),node.name)).collect::<Vec<_>>().join(",");
        format!("Self::build_prepared(ports,{args})")
    };
    let parameters = node
        .configuration
        .iter()
        .enumerate()
        .map(|(id, value)| format!("configuration{id}:{}", owned_type(&value.ty)))
        .collect::<Vec<_>>()
        .join(",");
    out.push(format!("fn build(ports:&mut hgl_describe::Ports<'_>)->Result<Self,hgl_describe::BuildError>{{{body}}}}}impl Node{index} {{fn build_prepared(ports:&mut hgl_describe::Ports<'_>,{parameters})->Result<Self,hgl_describe::BuildError>{{"));
}
fn node_type(n: &Node, index: usize, out: &mut Vec<String>) {
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
    out.push(format!(
        "uses_scheduler: {}, ..hgl_types::NodeType::default() }}\n}}\n",
        n.alarm
    ));
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
    let mut standalone = plan.clone();
    standalone.ordinary_instantiation = true;
    emit_inner(&standalone, true)
}
/// Emit a graph using nominal layouts already emitted in its enclosing module.
pub fn emit_shared(plan: &Plan) -> String {
    emit_inner(plan, false)
}
/// Share exact nominal layouts across independently constructed evaluation cases.
pub fn shared_layouts(plans: &[Plan]) -> String {
    let nodes = plans
        .iter()
        .flat_map(|plan| plan.nodes.iter().cloned())
        .collect();
    let types = Plan {
        nodes,
        ..Plan::default()
    };
    global_markers(&types)
        + &hgl_rust_preparation::recursive_markers(&types)
        + &hgl_rust_deltas::markers(&types)
}
fn emit_inner(plan: &Plan, layouts: bool) -> String {
    let prepared = hgl_rust_families::prepare(plan);
    let normalized = hgl_rust_direct_deltas::prepare(&prepared);
    let plan = &normalized;
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
    if let Some(error) = &plan.construction_error {
        out.push(format!("pub fn register(_: &mut hgl_describe::Registry) -> Result<(), hgl_describe::BuildError> {{ Ok(()) }}\npub fn main(_: &hgl_describe::Registry) -> Result<hgl_describe::GraphDescription, hgl_describe::BuildError> {{ Err(hgl_describe::BuildError::InvalidNodeType {{ node: \"hgl.program\", what: {error:?}.into() }}) }}\n"));
        return out.concat();
    }
    if layouts {
        out.push(global_markers(plan));
        out.push(hgl_rust_preparation::recursive_markers(plan));
        out.push(hgl_rust_deltas::markers(plan));
    }
    for (i, n) in plan.nodes.iter().enumerate() {
        node(plan, n, i, &mut out);
    }
    out.push("/// Register the selected HGL implementations.\npub fn register(registry: &mut hgl_describe::Registry) -> Result<(),hgl_describe::BuildError> {\n" .to_owned());
    for i in 0..plan.nodes.len() {
        out.push(format!("registry.register::<Node{i}>()?;\n"));
    }
    out.push("Ok(())\n}\n".into());
    out.push("/// Construct the checked entry graph without executing it.\npub fn main(registry: &hgl_describe::Registry) -> Result<hgl_describe::GraphDescription,hgl_describe::BuildError> {\nlet mut builder = hgl_describe::Builder::new(\"hgl.program\",registry);\n" .to_owned());
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
pub fn emit_test_body(plan: &Plan, expected: Option<&[Option<Value>]>) -> String {
    let prepared = hgl_rust_families::prepare(plan);
    let normalized = hgl_rust_direct_deltas::prepare(&prepared);
    let plan = &normalized;
    let mut out = vec![hgl_rust_keyed::preparation(plan), capacities(plan)];
    register_prepared(plan, &mut out);
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
    out.push("pub fn test()->Result<(),String> {\nlet mut registry=hgl_describe::Registry::new();\nlet configurations=prepare_configurations(&[])?;register_prepared(&mut registry,&configurations)?;\nlet graph=main(&registry).map_err(|e|format!(\"{e:?}\"))?;\n".into());
    if plan.construction_error.is_some() {
        out.push("Ok(())\n}\n".into());
        return out.concat();
    }
    out.push("let mut store=hgl_store::Store::new();\nstore.global_state().provision();prepare_static(&mut store.keys)?;\n".into());
    out.push(instantiate(plan, "0"));
    if let Some((key, ty)) = &plan.recording {
        out.push(format!(
            "let recording=store.global_state().bind::<{}>({key:?}).map_err(|e|format!(\"{{e:?}}\"))?;\n",
            global_type(ty)
        ));
    }
    out.push("store.prepare_collection_inputs();hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig { start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::MAX_END }).map_err(|e|format!(\"{e:?}\"))?;\n".into());
    if plan.output.is_some() {
        out.push(format!("let ticks=store.global_state().get(recording).map_err(|e|e.message)?.into_iter().collect();\ndrop(built);\nlet _observed=hgl_testkit::evaluation::observe(ticks,{})?;\n", plan.input_length));
        if let Some(expected) = expected {
            let (_, ty) = plan
                .output
                .as_ref()
                .unwrap_or_else(|| unreachable!("checked output"));
            let payload = ty
                .clone()
                .delta()
                .unwrap_or_else(|_| unreachable!("checked output delta"));
            let items = expected
                .iter()
                .map(|v| {
                    v.as_ref().map_or_else(
                        || "None".into(),
                        |v| format!("Some({})", condition_code(plan, v)),
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let comparison = if matches!(payload, Ty::Delta(_)) {
                format!(
                    "hgl_testkit::evaluation::compare_by(&expected,&_observed,|a,b|{})?;",
                    hgl_rust_deltas::equivalent(&payload, "a", "b")
                )
            } else {
                "hgl_testkit::evaluation::compare(&expected,&_observed)?;".into()
            };
            out.push(format!("let expected=(|| -> hgl_types::NodeResult<Vec<Option<{}>>> {{Ok(vec![{items}])}})().map_err(|e|e.message)?;\n{comparison}\n",owned_type(&payload)));
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
    if hgl_rust_deltas::shaped(ty) {
        return format!(
            "<{} as hgl_store::shapes::Shape>::shape()",
            hgl_rust_deltas::shape_marker(ty)
        );
    }
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

fn prepared_values(value: &Value, out: &mut std::collections::BTreeMap<usize, Ty>) {
    match &value.kind {
        Kind::Prepared(id) => {
            out.insert(*id, value.ty.clone());
        }
        Kind::List(values) | Kind::ValueCall(values, _) => {
            for v in values {
                prepared_values(v, out);
            }
        }
        Kind::Construct(fields) => {
            for (_, v) in fields {
                prepared_values(v, out);
            }
        }
        Kind::Delta(parts) => {
            for p in parts {
                for v in p.operands() {
                    prepared_values(v, out);
                }
            }
        }
        Kind::Field(v, _)
        | Kind::Length(v)
        | Kind::Unary(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v) => prepared_values(v, out),
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            prepared_values(a, out);
            prepared_values(b, out);
        }
        Kind::Captured(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::WiringFailure(_)
        | Kind::ObservedLocal(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::GlobalSet(..)
        | Kind::GeneratorLocal(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Native(..)
        | Kind::Query(..)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
}
fn node_prepared(node: &Node) -> bool {
    let mut values = std::collections::BTreeMap::new();
    for v in &node.configuration {
        prepared_values(v, &mut values);
    }
    !values.is_empty()
}
fn register_prepared(plan: &Plan, out: &mut Vec<String>) {
    let fields = plan
        .nodes
        .iter()
        .enumerate()
        .flat_map(|(i, node)| {
            node.configuration
                .iter()
                .enumerate()
                .map(move |(id, value)| {
                    format!("node{i}_configuration{id}:{},", owned_type(&value.ty))
                })
        })
        .collect::<Vec<_>>()
        .concat();
    out.push(format!("struct Configurations {{{fields}}}fn prepare_configurations(values:&[hgl_rust_ir::Value])->Result<Configurations,String>{{"));
    let mut bindings = std::collections::BTreeMap::new();
    for node in &plan.nodes {
        for value in &node.configuration {
            prepared_values(value, &mut bindings);
        }
    }
    for (id, ty) in bindings {
        out.push(format!(
            "let prepared{id}={};",
            hgl_rust_preparation::decode(
                &ty,
                &format!("values.get({id}).ok_or(\"missing prepared value\")?")
            )
        ));
    }
    out.push("Ok(Configurations {".into());
    for (i, node) in plan.nodes.iter().enumerate() {
        for (id, value) in node.configuration.iter().enumerate() {
            out.push(format!("node{i}_configuration{id}:(||->hgl_types::NodeResult<_>{{Ok({})}})().map_err(|e|e.message)?,",condition_code(plan,value)));
        }
    }
    out.push("})}fn register_prepared(registry:&mut hgl_describe::Registry,configurations:&Configurations)->Result<(),String>{".into());
    for (i, node) in plan.nodes.iter().enumerate() {
        if node.configuration.is_empty() {
            out.push(format!(
                "registry.register::<Node{i}>().map_err(|e|format!(\"{{e:?}}\"))?;"
            ));
            continue;
        }
        out.push("{".into());
        for (id, value) in node.configuration.iter().enumerate() {
            out.push(format!("let configuration{id}=<{} as hgl_store::GlobalValue>::retain(&configurations.node{i}_configuration{id}).map_err(|e|e.message)?;",global_type(&value.ty)));
        }
        let args=node.configuration.iter().enumerate().map(|(id,value)|format!("<{} as hgl_store::GlobalValue>::retain(&configuration{id}).map_err(|e|hgl_describe::BuildError::InvalidNodeType {{node:{:?},what:e.message}})?",global_type(&value.ty),node.name)).collect::<Vec<_>>().join(",");
        out.push(format!("registry.register_with(<Node{i} as hgl_describe::Buildable>::node_type(),move |ports|Node{i}::build_prepared(ports,{args})).map_err(|e|format!(\"{{e:?}}\"))?;}}"));
    }
    out.push("Ok(())}".into());
}

/// Emit fresh graph execution from already constructed owning arguments.
pub fn emit_prepared_test_body(plan: &Plan) -> String {
    let prepared = hgl_rust_families::prepare(plan);
    let normalized = hgl_rust_direct_deltas::prepare(&prepared);
    let plan = &normalized;
    let legacy = emit_test_body(plan, None);
    let adapters = legacy.split("pub fn test()").next().unwrap_or_default();
    let mut out=vec![adapters.into(),"pub fn run(prepared:hgl_harness_ir::PreparedEval)->Result<hgl_harness_ir::CapturedEval,hgl_harness_ir::Failure> {\nlet mut registry=hgl_describe::Registry::new();\n".into()];
    if plan.construction_error.is_some() {
        out.push("register(&mut registry).map_err(|e|format!(\"{e:?}\"))?; main(&registry).map_err(|e|format!(\"{e:?}\"))?; Ok(hgl_harness_ir::CapturedEval {length:prepared.input_length,ticks:Vec::new()}) }\n".into());
        return out.concat();
    }
    out.push("let configurations=prepare_configurations(&prepared.arguments)?;register_prepared(&mut registry,&configurations)?;".into());
    out.push("let graph=main(&registry).map_err(|e|format!(\"{e:?}\"))?;\nlet mut store=hgl_store::Store::new();store.global_state().provision();prepare_static(&mut store.keys)?;prepare_values(&mut store.keys,&prepared.arguments)?;\n".into());
    out.push(instantiate(plan, "prepared.input_length"));
    if let Some((key, ty)) = &plan.recording {
        out.push(format!("let recording=store.global_state().bind::<{}>({key:?}).map_err(|e|format!(\"{{e:?}}\"))?;\n",global_type(ty)));
    }
    out.push("store.prepare_collection_inputs();hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig {start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::MAX_END}).map_err(|e|match e {hgl_kernel::EngineError::Node(e)=>hgl_harness_ir::Failure::Execution(e),hgl_kernel::EngineError::BadTimes=>hgl_harness_ir::Failure::Other(\"invalid run times\".into())})?;\n".into());
    if let Some((_, ty)) = &plan.output {
        let payload = ty
            .clone()
            .delta()
            .unwrap_or_else(|_| unreachable!("checked output delta"));
        out.push(format!("let ticks=store.global_state().get(recording).map_err(|e|e.message)?.into_iter().collect();drop(built);\nlet observed=hgl_testkit::evaluation::observe(ticks,prepared.input_length)?;\nlet (length,ticks)=observed.into_parts(); Ok(hgl_harness_ir::CapturedEval {{length,ticks:ticks.iter().map(|(i,v)|(*i,{})).collect()}})\n",hgl_rust_preparation::encode(&payload,"v")));
    } else {
        out.push(
            "Ok(hgl_harness_ir::CapturedEval {length:prepared.input_length,ticks:Vec::new()})\n"
                .into(),
        );
    }
    out.push("}\n".into());
    out.concat()
}
fn capacities(plan: &Plan) -> String {
    if !hgl_rust_finite_domains::prepared(plan) {
        return String::new();
    }
    let Some(arrivals) = hgl_rust_finite_domains::direct_arrivals(plan) else {
        return String::new();
    };
    let capacity = hgl_rust_capacity::Capacity::new(plan);
    let mut code = capacity.declaration();
    code += "fn prepare_capacity(configurations:&Configurations,store:&mut hgl_store::Store,horizon:usize)->Result<FiniteCapacity,String>{let mut capacity=FiniteCapacity::default();let mut cycles=horizon;";
    append(
        &mut code,
        format_args!(
            "cycles=cycles.checked_add({arrivals}usize).ok_or(\"finite arrival count overflow\")?;"
        ),
    );
    for (i, node) in plan.nodes.iter().enumerate() {
        for (id, value) in node.configuration.iter().enumerate() {
            append(
                &mut code,
                format_args!(
                    "{{let value=&configurations.node{i}_configuration{id};{}",
                    capacity.include(&value.ty, "value")
                ),
            );
            if matches!(value.ty, Ty::List(..)) {
                code += "cycles=cycles.saturating_add(value.len());";
            }
            code += "}";
        }
    }
    code += &capacity.constants(plan, |value| condition_code(plan, value));
    code += &capacity.scalar_sets(plan);
    code += &capacity.native_limits(plan);
    code += &capacity.text_limits(plan);
    for node in &plan.nodes {
        for (key, ty) in &node.globals {
            append(
                &mut code,
                format_args!("{{let mut bounds={};", capacity.bounds(ty)),
            );
            if plan
                .recording
                .as_ref()
                .is_some_and(|(record, _)| record == key)
            {
                code += "bounds.len=cycles.saturating_add(1);";
            }
            append(
                &mut code,
                format_args!(
                    "store.global_state().prepare_value::<{}>({key:?},&bounds).map_err(|e|e.message)?;}}",
                    global_type(ty)
                ),
            );
        }
    }
    code += "capacity.arrivals=cycles;Ok(capacity)}\nfn prepare_outputs(capacity:&FiniteCapacity,store:&mut hgl_store::Store,graph:&hgl_describe::GraphDescription,built:&hgl_describe::BuiltGraph)->hgl_types::NodeResult {let empty=FiniteTopology::default();";
    for (i, node) in plan.nodes.iter().enumerate() {
        if node.result.publication() {
            code += &capacity.output(
                &node.result,
                &format!("built.outputs[graph.nodes.iter().position(|node|node.implementation=={:?}).expect(\"checked node\")].expect(\"checked output\")",format!("{}#{i}",node.name)),
            );
        }
    }
    code += "Ok(())}\n";
    code
}

fn instantiate(plan: &Plan, horizon: &str) -> String {
    let finite = hgl_rust_finite_domains::prepared(plan);
    let mut code = if finite {
        format!("let capacity=prepare_capacity(&configurations,&mut store,{horizon})?;")
    } else {
        String::new()
    };
    code += "let mut built=hgl_describe::instantiate_complete(&graph,&registry,&mut store).map_err(|e|format!(\"{e:?}\"))?;\n";
    if finite {
        code += "prepare_outputs(&capacity,&mut store,&graph,&built).map_err(|e|e.message)?;\n";
    }
    code
}
