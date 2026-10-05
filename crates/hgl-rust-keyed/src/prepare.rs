use hgl_rust_ir::{Plan, Value};
use hgl_rust_key_origins::node_keys;
use std::fmt::Write as _;
/// Seed literal domains when a generated node is constructed outside the eval harness.
pub fn node_preparation(node: &hgl_rust_ir::Node, emit: impl Fn(&Value) -> String) -> String {
    let keys = node_keys(node);
    keys.iter().map(|key| format!(
        "<{} as hgl_store::Key>::prepare(ports.keys(),&({})).map_err(|e|hgl_describe::BuildError::InvalidNodeType {{node:{:?},what:e.message}})?;",
        hgl_rust_layouts::global_type(&key.ty),emit(key),node.name
    )).collect::<Vec<_>>().concat()
}
/// Emit exclusively cold traversal and static domain preparation for this plan.
pub fn preparation(plan: &Plan) -> String {
    let keys = plan.nodes.iter().flat_map(node_keys).collect::<Vec<_>>();
    let constants = keys
        .iter()
        .map(|key| format!("prepare_key(keys,&{})?;", hgl_rust_checked_data::value(key)))
        .collect::<Vec<_>>()
        .concat();
    let mut code = format!(
        "fn prepare_static(keys:&mut hgl_store::Keys)->Result<(),String> {{{constants} Ok(())}}\n"
    );
    code +=
        "fn prepare_key(keys:&mut hgl_store::Keys,value:&hgl_rust_ir::Value)->Result<(),String> {";
    for ty in hgl_rust_layouts::global_types(plan).values() {
        if let hgl_source::Ty::Struct(name, fields, _) = ty
            && ty.collection_key()
        {
            let ty = if name.source_name().starts_with("\0tuple<") {
                hgl_source::Ty::Tuple(fields.iter().map(|(_, ty)| ty.clone()).collect())
            } else {
                ty.clone()
            };
            write!(code,
                "if value.ty=={} {{return <{} as hgl_store::Key>::prepare(keys,&({})).map_err(|e|e.message);}}",
                hgl_rust_checked_data::ty(&ty),
                hgl_rust_layouts::global_type(&ty),
                hgl_rust_value_convert::decode(&ty, "value")
            ).unwrap_or_else(|_| unreachable!("String formatting"));
        }
    }
    code += r#"let hgl_rust_ir::Kind::Literal(value)=&value.kind else {return Err("unmaterialized collection key".into())}; match value {"#;
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
