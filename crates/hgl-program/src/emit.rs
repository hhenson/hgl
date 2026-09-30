use crate::resolve::{Kind, Node, Plan, Statement, Value};
use crate::syntax::{Literal, Ty};

fn rust_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::I64 | Ty::Duration => "i64",
        Ty::Bool => "bool",
        Ty::Str => "String",
        Ty::Void => "()",
    }
}
fn scalar_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::Bool => "Bool",
        Ty::I64 => "I64",
        Ty::Str | Ty::Duration | Ty::Void => unreachable!("checked endpoint type"),
    }
}
fn literal(value: &Literal) -> String {
    match value {
        Literal::Int(i) | Literal::Duration(i) => format!("{i}_i64"),
        Literal::Bool(b) => b.to_string(),
        Literal::Str(s) => format!("{s:?}.to_owned()"),
    }
}
fn value(plan: &Plan, v: &Value) -> String {
    match &v.kind {
        Kind::Literal(l) => literal(l),
        Kind::Input(i) => format!("_ctx.get(self.input{i})"),
        Kind::Cache(i) => format!("self.cache{i}"),
        Kind::Local(i) => format!("local{i}.clone()"),
        Kind::Native(i, args) => format!(
            "<crate::Provider as Native>::{}({})",
            plan.natives[*i].method,
            args.iter()
                .map(|v| value(plan, v))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Kind::Binary(op, a, b) => {
            let a = value(plan, a);
            let b = value(plan, b);
            if op == "+" && v.ty == Ty::Str {
                format!("format!(\"{{}}{{}}\", {a}, {b})")
            } else {
                format!("({a} {op} {b})")
            }
        }
        Kind::Wire(_) | Kind::Void => unreachable!("checked runtime value"),
    }
}
fn statements(plan: &Plan, body: &[Statement], out: &mut Vec<String>) {
    for statement in body {
        out.push(match statement {
            Statement::Let(i, v) => format!("let local{i} = {};\n", value(plan, v)),
            Statement::Return(v) => format!(
                "_ctx.set(self.output, {});\nreturn Ok(());\n",
                value(plan, v)
            ),
            Statement::Call(v) => format!("{};\n", value(plan, v)),
            Statement::Add(i, v) => format!("self.cache{i} += {};\n", value(plan, v)),
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
    for (i, (_, _, ty)) in n.inputs.iter().enumerate() {
        out.push(format!("input{i}: hgl_store::In<{}>,\n", rust_type(ty)));
    }
    for (i, cache) in n.caches.iter().enumerate() {
        out.push(format!("cache{i}: {},\n", rust_type(&cache.ty())));
    }
    if n.result != Ty::Void {
        out.push(format!(
            "output: hgl_store::Out<{}>,\n",
            rust_type(&n.result)
        ));
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
    let guard = if n.inputs.is_empty() {
        n.alarm.to_string()
    } else {
        n.inputs
            .iter()
            .enumerate()
            .map(|(i, _)| format!("_ctx.modified(self.input{i})"))
            .collect::<Vec<_>>()
            .join(" || ")
    };
    out.push(format!("if {guard} {{\n"));
    statements(plan, &n.statements, out);
    out.push("}\nOk(())\n}\n}\n".to_owned());
    out.push(format!("impl hgl_describe::Buildable for Node{index} {{\nfn node_type() -> hgl_types::NodeType {{\nhgl_types::NodeType {{ name: {:?}, inputs: vec![",format!("{}#{index}",n.name)));
    for (name, _, ty) in &n.inputs {
        out.push(format!(
            "({name:?},hgl_types::TsType::Ts(hgl_types::ScalarType::{})),",
            scalar_type(ty)
        ));
    }
    out.push("],\n".to_owned());
    if n.result != Ty::Void {
        out.push(format!(
            "output: Some(hgl_types::TsType::Ts(hgl_types::ScalarType::{})),\n",
            scalar_type(&n.result)
        ));
    }
    out.push(format!("uses_scheduler: {}, ..hgl_types::NodeType::default() }}\n}}\nfn build(ports: &mut hgl_describe::Ports<'_>) -> Result<Self,hgl_describe::BuildError> {{\nOk(Self {{\n",n.alarm));
    for (i, (name, _, _)) in n.inputs.iter().enumerate() {
        out.push(format!("input{i}: ports.input({name:?})?,\n"));
    }
    for (i, cache) in n.caches.iter().enumerate() {
        out.push(format!("cache{i}: {},\n", literal(cache)));
    }
    if n.result != Ty::Void {
        out.push("output: ports.output()?,\n".to_owned());
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
    out.push("/// Selected native scalar interfaces.\npub trait Native {\n".to_owned());
    for native in &plan.natives {
        out.push(format!(
            "/// HGL `{}`.\nfn {}({}) -> {};\n",
            native.name,
            native.method,
            native
                .args
                .iter()
                .enumerate()
                .map(|(i, ty)| format!("arg{i}: {}", rust_type(ty)))
                .collect::<Vec<_>>()
                .join(", "),
            rust_type(&native.result)
        ));
    }
    out.push("}\n".to_owned());
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
    if matches!(condition.kind, Kind::Binary(_, _, _)) {
        code[1..code.len() - 1].into()
    } else {
        code
    }
}
