use crate::resolve::{Kind, Node, Plan, Statement, Value};
use crate::syntax::{Literal, Ty};

fn rust_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::I64 => "i64",
        Ty::Duration => "hgl_types::EngineDelta",
        Ty::Date => "hgl_types::Date",
        Ty::Time => "hgl_types::Time",
        Ty::DateTime => "hgl_types::EngineTime",
        Ty::Bool => "bool",
        Ty::F64 => "f64",
        Ty::Str => "String",
        Ty::Void => "()",
        Ty::Ref(_) => "hgl_store::Reference",
        Ty::Set(_) => "hgl_store::InputId",
    }
}
fn scalar_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::Bool => "Bool",
        Ty::F64 => "F64",
        Ty::I64 => "I64",
        Ty::Str => "Text",
        Ty::Duration => "Duration",
        Ty::Date => "Date",
        Ty::Time => "Time",
        Ty::DateTime => "DateTime",
        Ty::Ref(_) | Ty::Set(_) | Ty::Void => unreachable!("checked endpoint type"),
    }
}
fn literal(value: &Literal) -> String {
    match value {
        Literal::Int(i) => format!("{i}_i64"),
        Literal::Duration(i) => format!("hgl_types::EngineDelta::from_micros({i})"),
        Literal::Date(i) => format!("hgl_types::Date({i})"),
        Literal::Time(i) => format!("hgl_types::Time({i})"),
        Literal::DateTime(i) => format!("hgl_types::EngineTime::from_micros({i})"),
        Literal::Bool(b) => b.to_string(),
        Literal::Float(f) => format!("{f:?}_f64"),
        Literal::Str(s) => format!("{s:?}.to_owned()"),
    }
}
fn value(plan: &Plan, v: &Value) -> String {
    match &v.kind {
        Kind::Literal(l) => literal(l),
        Kind::Input(i) => {
            if matches!(v.ty, Ty::Ref(_)) {
                format!("_ctx.store().bindings().input_reference(self.input{i})")
            } else {
                format!("_ctx.get(self.input{i})")
            }
        }
        Kind::Cache(i) => {
            if v.ty == Ty::Str {
                format!("self.cache{i}.clone()")
            } else {
                format!("self.cache{i}")
            }
        }
        Kind::Local(i) => format!("local{i}.clone()"),
        Kind::Native(i, args) => format!(
            "<crate::Provider as Native>::{}({}){}",
            plan.natives[*i].method,
            args.iter()
                .map(|v| native_argument(plan, v))
                .collect::<Vec<_>>()
                .join(", "),
            if plan.natives[*i].throws { "?" } else { "" }
        ),
        Kind::Binary(op, a, b) => {
            let a = if a.ty == Ty::Str {
                native_argument(plan, a)
            } else {
                value(plan, a)
            };
            let b = if b.ty == Ty::Str {
                native_argument(plan, b)
            } else {
                value(plan, b)
            };
            if v.ty == Ty::I64 && matches!(op.as_str(), "+" | "-" | "*" | "%") {
                integer_binary(op, &a, &b)
            } else if op == "/" {
                format!(
                    "{{ let lhs = ({a}) as f64; let rhs = ({b}) as f64; if rhs == 0.0 {{ return Err(hgl_kernel::NodeError::new(\"division by zero\")); }} lhs / rhs }}"
                )
            } else if op == "+" && v.ty == Ty::Str {
                format!("format!(\"{{}}{{}}\", {a}, {b})")
            } else {
                format!("({a} {op} {b})")
            }
        }
        Kind::Unary(op, v) if op == "-" && v.ty == Ty::I64 => {
            format!("(({}).wrapping_neg())", value(plan, v))
        }
        Kind::Unary(op, v) => {
            let v = value(plan, v);
            if op == "float" {
                format!("({v} as f64)")
            } else {
                format!("({op}{v})")
            }
        }
        Kind::Query(op, args) => {
            if op.starts_with("set_") {
                set_query(plan, op, args)
            } else {
                query(op, args)
            }
        }
        Kind::Output => "_ctx.output_value(self._output).expect(\"valid output\")".into(),
        Kind::Wire(_) | Kind::Void => unreachable!("checked runtime value"),
    }
}
fn integer_binary(op: &str, a: &str, b: &str) -> String {
    match op {
        "+" => format!("(({a}).wrapping_add({b}))"),
        "-" => format!("(({a}).wrapping_sub({b}))"),
        "*" => format!("(({a}).wrapping_mul({b}))"),
        "%" => format!(
            "{{ let lhs = {a}; let rhs = {b}; if rhs == -1 {{ 0_i64 }} else {{ let rem = lhs.checked_rem(rhs).ok_or_else(|| hgl_kernel::NodeError::new(\"modulo by zero\"))?; if rem != 0 && (rem < 0) != (rhs < 0) {{ rem + rhs }} else {{ rem }} }} }}"
        ),
        _ => unreachable!("checked integer operation"),
    }
}

fn statements(plan: &Plan, body: &[Statement], out: &mut Vec<String>) {
    for statement in body {
        out.push(match statement {
            Statement::Let(i, v) => format!("let local{i} = {};\n", condition_code(plan, v)),
            Statement::Return(v) => if matches!(v.ty,Ty::Ref(_)) {format!("_ctx.set_reference(self._output,{})?;\nreturn Ok(());\n",condition_code(plan,v))} else {format!("_ctx.set(self._output, {});\nreturn Ok(());\n",condition_code(plan,v))},
            Statement::Call(v) => format!("{};\n", condition_code(plan, v)),
            Statement::For(id,collection,body)=> {
                let Kind::Input(input)=collection.kind else {unreachable!("checked collection")};
                let Ty::Set(element)=&collection.ty else {unreachable!("checked collection")};
                let mut code=vec![format!("let mut index{id}=0;\nwhile let Some(key{id})=_ctx.store().bindings().changed_keys(self.input{input}).get(index{id}).copied() {{\nindex{id}+=1;\nif _ctx.store().bindings().input(self.input{input}).members.initial.get(&key{id})==Some(&false) && _ctx.store().bindings().child_input(self.input{input},key{id}).is_some() {{\nlet local{id}={};\n",if **element==Ty::Bool {format!("key{id}!=0")} else {format!("key{id}")})];
                statements(plan,body,&mut code);code.push("}\n}\n".into());code.concat()
            }
            Statement::Assign(target,v)=> if matches!(target.kind,Kind::Output) { format!("_ctx.set(self._output, {});\n",condition_code(plan,v)) } else {let Kind::Cache(i)=target.kind else {unreachable!("checked assignment")}; format!("self.cache{i} = {};\n",condition_code(plan,v))},
            Statement::Add(i, v) => format!("self.cache{i} = self.cache{i}.wrapping_add({});\n", condition_code(plan, v)),
            Statement::If(condition, yes, no) => {
                let mut code = vec![format!("if {} {{\n", condition_code(plan, condition))];
                statements(plan, yes, &mut code);
                code.push("} else {\n".to_owned());
                statements(plan, no, &mut code);
                code.push("}\n".to_owned());
                code.concat()
            }
        });
    }
}
fn node(plan: &Plan, n: &Node, index: usize, out: &mut Vec<String>) {
    out.push(format!("#[derive(Debug)]\nstruct Node{index} {{\n"));
    if n.replay.is_some() {
        out.push("next: usize,\n".into());
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
    for delay in &n.delays {
        out.push(format!(
            "_ctx.alarm_in(hgl_types::EngineDelta::from_micros({delay}))?;\n"
        ));
    }
    out.push("Ok(())\n}\nfn eval(&mut self, _ctx: &mut hgl_kernel::Ctx<'_>) -> hgl_kernel::NodeResult {\n" .to_owned());
    node_eval(plan, n, out);
    node_build(n, index, out);
}
fn node_eval(plan: &Plan, n: &Node, out: &mut Vec<String>) {
    let guard = if n.inputs.is_empty() {
        n.alarm.to_string()
    } else {
        "true".into()
    };
    out.push(format!("if {guard} {{\n"));
    if let Some(ticks) = &n.replay {
        out.push("match self.next {\n".into());
        for (i, tick) in ticks.iter().enumerate() {
            if let Some(tick) = tick {
                out.push(format!(
                    "{i} => _ctx.set(self._output, {}),\n",
                    literal(tick)
                ));
            }
        }
        out.push(format!("_ => {{}},\n}}\nself.next += 1;\nif self.next < {} {{ _ctx.schedule_in(hgl_types::EngineDelta::STEP)?; }}\n",ticks.len()));
    } else {
        for (condition, body) in &n.handlers {
            let guard = condition
                .as_ref()
                .map_or_else(|| input_guard(n), |v| condition_code(plan, v));
            out.push(format!("if {guard} {{\n"));
            statements(plan, body, out);
            out.push("}\n".into());
        }
    }
    out.push("}\nOk(())\n}\n}\n".to_owned());
}
fn node_build(n: &Node, index: usize, out: &mut Vec<String>) {
    out.push(format!("impl hgl_describe::Buildable for Node{index} {{\nfn node_type() -> hgl_types::NodeType {{\nhgl_types::NodeType {{ name: {:?}, inputs: vec![",format!("{}#{index}",n.name)));
    for (name, _, ty) in &n.inputs {
        out.push(format!("({name:?},{}),", shape(ty)));
    }
    out.push("],\n".to_owned());
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
    if n.replay.is_some() {
        out.push("next: 0,\n".into());
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
pub(crate) fn emit(plan: &Plan) -> String {
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

fn condition_code(plan: &Plan, condition: &Value) -> String {
    let code = value(plan, condition);
    if code.starts_with('(') && code.ends_with(')') {
        code[1..code.len() - 1].into()
    } else {
        code
    }
}

pub(crate) fn test_body(plan: &Plan, expected: Option<&[Option<Literal>]>) -> String {
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
    if let Some((output, ty)) = &plan.output {
        let identity = format!("{}#{output}", plan.nodes[*output].name);
        out.push(format!("let output=u32::try_from(graph.nodes.iter().position(|n|n.implementation=={identity:?}).ok_or(\"missing eval output\")?).map_err(|e|e.to_string())?;\nlet _observed=hgl_testkit::evaluation::evaluate::<{}>(graph,&mut registry,output,{})?;\n",rust_type(ty),plan.input_length));
        if let Some(expected) = expected {
            let values = expected
                .iter()
                .map(|l| {
                    l.as_ref()
                        .map_or_else(|| "None".to_owned(), |l| format!("Some({})", literal(l)))
                })
                .collect::<Vec<_>>()
                .join(",");
            out.push(format!(
                "hgl_testkit::evaluation::compare(&[{values}],&_observed)?;\n"
            ));
        }
    } else {
        out.push("let mut store=hgl_store::Store::new();\nlet mut built=hgl_describe::instantiate_complete(&graph,&registry,&mut store).map_err(|e|format!(\"{e:?}\"))?;\nhgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig { start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::MAX_END }).map_err(|e|format!(\"{e:?}\"))?;\n".into());
    }
    out.push("Ok(())\n}\n".into());
    out.concat()
}

fn query(op: &str, args: &[Value]) -> String {
    let values = args
        .iter()
        .map(|v| match &v.kind {
            Kind::Input(i) if matches!(v.ty, Ty::Ref(_) | Ty::Set(_)) => match op {
                "valid" => format!("_ctx.store().input_valid(self.input{i})"),
                "modified" => format!(
                    "_ctx.store().bindings().modified(self.input{i},_ctx.evaluation_time())"
                ),
                "last_modified" => format!("_ctx.store().bindings().last_modified(self.input{i})"),
                _ => unreachable!("unsupported structural query"),
            },
            Kind::Input(i) => match op {
                "last_modified" => format!("_ctx.last_modified(self.input{i})"),
                "activate" | "passivate" => {
                    format!("_ctx.set_active(self.input{i},{})", op == "activate")
                }
                _ => format!("_ctx.{op}(self.input{i})"),
            },
            Kind::Output => match op {
                "valid" => "_ctx.store().output_ref(self._output).is_some()".into(),
                "last_modified" => {
                    "_ctx.store().bindings().output(self._output.id()).modified_at".into()
                }
                "modified" => {
                    "_ctx.store().output_modified(self._output.id(),_ctx.evaluation_time())".into()
                }
                _ => unreachable!("checked output query"),
            },
            Kind::Literal(_)
            | Kind::Wire(_)
            | Kind::Cache(_)
            | Kind::Local(_)
            | Kind::Native(..)
            | Kind::Binary(..)
            | Kind::Unary(..)
            | Kind::Query(..)
            | Kind::Void => unreachable!("checked endpoint query"),
        })
        .collect::<Vec<_>>();
    if values.len() == 1 {
        values[0].clone()
    } else {
        format!(
            "({})",
            values.join(if op == "modified" { " || " } else { " && " })
        )
    }
}

fn native_result(native: &crate::resolve::Native) -> String {
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
    match ty {
        Ty::Ref(t) => format!("hgl_types::TsType::Reference(Box::new({}))", shape(t)),
        Ty::Set(t) => format!(
            "hgl_types::TsType::Set(hgl_types::ScalarType::{})",
            scalar_type(t)
        ),
        Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Void => format!(
            "hgl_types::TsType::Ts(hgl_types::ScalarType::{})",
            scalar_type(ty)
        ),
    }
}

fn set_query(plan: &Plan, op: &str, args: &[Value]) -> String {
    let endpoint = match args[0].kind {
        Kind::Output => "self._output".to_owned(),
        Kind::Input(i) => format!("self.input{i}"),
        Kind::Literal(_)
        | Kind::Wire(_)
        | Kind::Cache(_)
        | Kind::Local(_)
        | Kind::Native(..)
        | Kind::Binary(..)
        | Kind::Unary(..)
        | Kind::Query(..)
        | Kind::Void => unreachable!("checked set"),
    };
    if op == "set_bound" {
        return format!("_ctx.store().bindings().has_peer({endpoint})");
    }
    if op == "set_len" {
        return format!(
            "i64::try_from(_ctx.store().bindings().input({endpoint}).members.live.len()).map_err(|e|hgl_kernel::NodeError::new(e.to_string()))?"
        );
    }
    let key = condition_code(plan, &args[1]);
    let key = if args[1].ty == Ty::Bool {
        format!("i64::from({key})")
    } else {
        key
    };
    match op {
        "set_contains" => {
            format!("_ctx.store().bindings().child_input({endpoint},{key}).is_some()")
        }
        "set_discard" => format!("_ctx.remove_shaped({endpoint},{key})"),
        "set_upsert" => format!(
            "{{let key={key}; if _ctx.store().bindings().child_output({endpoint},key).is_none() {{let child=_ctx.get_or_create_shaped({endpoint},key);let child=_ctx.store().scalar_output::<bool>(child).map_err(|e|hgl_kernel::NodeError::new(format!(\"{{e:?}}\")))?;_ctx.set(child,true);}}}}"
        ),
        _ => unreachable!("checked set operation"),
    }
}

fn input_guard(n: &Node) -> String {
    if n.inputs.is_empty() {
        return n.alarm.to_string();
    }
    n.inputs
        .iter()
        .enumerate()
        .map(|(i, (_, _, ty))| {
            query(
                "valid",
                &[Value {
                    ty: ty.clone(),
                    kind: Kind::Input(i),
                }],
            )
        })
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
fn native_argument(plan: &Plan, v: &Value) -> String {
    if v.ty != Ty::Str {
        return condition_code(plan, v);
    }
    match &v.kind {
        Kind::Input(i) => format!("_ctx.store().get_ref(self.input{i}).as_str()"),
        Kind::Literal(Literal::Str(s)) => format!("{s:?}"),
        Kind::Cache(i) => format!("self.cache{i}.as_str()"),
        Kind::Local(i) => format!("local{i}.as_str()"),
        Kind::Output => {
            "_ctx.store().output_ref(self._output).expect(\"valid output\").as_str()".into()
        }
        Kind::Literal(_)
        | Kind::Wire(_)
        | Kind::Native(..)
        | Kind::Binary(..)
        | Kind::Unary(..)
        | Kind::Query(..)
        | Kind::Void => format!("&({})", value(plan, v)),
    }
}
