use hgl_rust_ir::{DeltaEntry, Kind, Plan, Statement, Value};
/// Seed literal domains when a generated node is constructed outside the eval harness.
pub fn node_preparation(node: &hgl_rust_ir::Node, emit: impl Fn(&Value) -> String) -> String {
    let mut keys = Vec::new();
    for value in &node.configuration {
        collect(value, &mut keys);
    }
    statements(&node.start, &mut keys);
    statements(&node.stop, &mut keys);
    if let Some(body) = &node.generator {
        statements(body, &mut keys);
    }
    for (guard, body) in &node.handlers {
        if let Some(guard) = guard {
            collect(guard, &mut keys);
        }
        statements(body, &mut keys);
    }
    keys.iter().map(|key| format!(
        "<{} as hgl_store::Key>::prepare(ports.keys(),&({})).map_err(|e|hgl_describe::BuildError::InvalidNodeType {{node:{:?},what:e.message}})?;",
        hgl_rust_layouts::global_type(&key.ty),emit(key),node.name
    )).collect::<Vec<_>>().concat()
}
/// Emit exclusively cold traversal and static domain preparation for this plan.
pub fn preparation(plan: &Plan) -> String {
    let mut keys = Vec::new();
    for node in &plan.nodes {
        for value in &node.configuration {
            collect(value, &mut keys);
        }
        statements(&node.start, &mut keys);
        statements(&node.stop, &mut keys);
        if let Some(body) = &node.generator {
            statements(body, &mut keys);
        }
        for (guard, body) in &node.handlers {
            if let Some(guard) = guard {
                collect(guard, &mut keys);
            }
            statements(body, &mut keys);
        }
    }
    let constants = keys
        .iter()
        .map(|key| format!("prepare_key(keys,&{})?;", hgl_rust_checked_data::value(key)))
        .collect::<Vec<_>>()
        .concat();
    let mut code = format!(
        "fn prepare_static(keys:&mut hgl_store::Keys)->Result<(),String> {{{constants} Ok(())}}\n"
    );
    code += r#"fn prepare_key(keys:&mut hgl_store::Keys,value:&hgl_rust_ir::Value)->Result<(),String> {let hgl_rust_ir::Kind::Literal(value)=&value.kind else {return Err("unmaterialized collection key".into())}; match value {"#;
    code += "hgl_source::Literal::Bool(v)=><bool as hgl_store::Key>::prepare(keys,v),";
    code += "hgl_source::Literal::Int(v)=><i64 as hgl_store::Key>::prepare(keys,v),";
    code += "hgl_source::Literal::Float(v)=><f64 as hgl_store::Key>::prepare(keys,v),";
    code += "hgl_source::Literal::Str(v)=><String as hgl_store::Key>::prepare(keys,v),";
    code += "hgl_source::Literal::Date(v)=><hgl_types::Date as hgl_store::Key>::prepare(keys,&hgl_types::Date(*v)),";
    code += "hgl_source::Literal::Time(v)=><hgl_types::Time as hgl_store::Key>::prepare(keys,&hgl_types::Time(*v)),";
    code += "hgl_source::Literal::Duration(v)=><hgl_types::EngineDelta as hgl_store::Key>::prepare(keys,&hgl_types::EngineDelta::from_micros(*v)),";
    code += "hgl_source::Literal::DateTime(v)=><hgl_types::EngineTime as hgl_store::Key>::prepare(keys,&hgl_types::EngineTime::from_micros(*v)),";
    code += "hgl_source::Literal::CivilDateTime(v)=><hgl_types::CivilDateTime as hgl_store::Key>::prepare(keys,&hgl_types::CivilDateTime::from_micros(*v)),";
    code +=
        "hgl_source::Literal::TimeZone(v)=><hgl_types::ZoneId as hgl_store::Key>::prepare(keys,v),";
    code += "hgl_source::Literal::ZonedTime(v)=><hgl_types::ZonedTime as hgl_store::Key>::prepare(keys,v),";
    code += "hgl_source::Literal::ZonedDateTime(v)=><hgl_types::ZonedDateTime as hgl_store::Key>::prepare(keys,v),";
    code += "hgl_source::Literal::Enum(_,v)=><i64 as hgl_store::Key>::prepare(keys,v),}.map_err(|e|e.message)}\n";
    code += r"fn prepare_values(keys:&mut hgl_store::Keys,values:&[hgl_rust_ir::Value])->Result<(),String> {for value in values {prepare_value(keys,value)?;}Ok(())}
fn prepare_value(keys:&mut hgl_store::Keys,value:&hgl_rust_ir::Value)->Result<(),String> {
match &value.kind {
hgl_rust_ir::Kind::List(values)=>prepare_values(keys,values)?,
hgl_rust_ir::Kind::Construct(values)=>for (_,value) in values {prepare_value(keys,value)?;},
hgl_rust_ir::Kind::Delta(parts)=>for part in parts {match part {
hgl_rust_ir::DeltaEntry::Add(k)|hgl_rust_ir::DeltaEntry::Remove(k)=>prepare_key(keys,k)?,
hgl_rust_ir::DeltaEntry::Keyed(k,v)=>{prepare_key(keys,k)?;prepare_value(keys,v)?;},
hgl_rust_ir::DeltaEntry::Child(_,v)=>prepare_value(keys,v)?,
}},_=>{}
}Ok(())}
";
    code
}
fn collect<'a>(value: &'a Value, keys: &mut Vec<&'a Value>) {
    match &value.kind {
        Kind::Delta(parts) => {
            for part in parts {
                match part {
                    DeltaEntry::Add(k) | DeltaEntry::Remove(k) | DeltaEntry::Keyed(k, _) => {
                        if matches!(k.kind, Kind::Literal(_)) {
                            keys.push(k);
                        }
                    }
                    DeltaEntry::Child(..) => {}
                }
                for value in part.operands() {
                    collect(value, keys);
                }
            }
        }
        Kind::List(values) | Kind::Native(_, values) | Kind::Query(_, values) => {
            for value in values {
                collect(value, keys);
            }
        }
        Kind::Construct(values) => {
            for (_, value) in values {
                collect(value, keys);
            }
        }
        Kind::ValueCall(values, body) => {
            for value in values {
                collect(value, keys);
            }
            statements(body, keys);
        }
        Kind::Field(v, _)
        | Kind::GlobalSet(_, v)
        | Kind::Length(v)
        | Kind::Unary(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v) => collect(v, keys),
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            collect(a, keys);
            collect(b, keys);
        }
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
}
fn statements<'a>(body: &'a [Statement], keys: &mut Vec<&'a Value>) {
    for statement in body {
        match statement {
            Statement::Let(_, v)
            | Statement::Var(_, v)
            | Statement::Borrow(_, v, _)
            | Statement::Return(v)
            | Statement::Yield(v)
            | Statement::Call(v) => collect(v, keys),
            Statement::TimedYield(a, b) | Statement::Assign(a, b) => {
                collect(a, keys);
                collect(b, keys);
            }
            Statement::While(v, body) | Statement::For(_, v, body) => {
                collect(v, keys);
                statements(body, keys);
            }
            Statement::If(v, a, b) => {
                collect(v, keys);
                statements(a, keys);
                statements(b, keys);
            }
            Statement::Exit => {}
        }
    }
}
