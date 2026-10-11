//! Typed finite recursion at cold checked/native value conversion boundaries.
use crate::layouts::{global_type, global_types};
use hgl_semantics::ir::Plan;
use hgl_source::Ty;
/// Emit statically selected recursive converters for every complete reachable batch.
pub fn markers(
    plan: &Plan,
    decode: fn(&Ty, &str) -> String,
    encode: fn(&Ty, &str) -> String,
) -> String {
    global_types(plan).values().filter_map(|ty| {
        if !matches!(ty,Ty::Recursive(_)) { return None; }
        let (_,fields,optional)=ty.structure().unwrap_or_else(|_|unreachable!("complete collected batch"));
        let name=global_type(ty);
        let values=fields.iter().enumerate().map(|(i,(_,child))| {
            let found=format!("items.iter().find(|(id,_)|*id=={i}).map(|(_,v)|v)");
            if optional.contains(&i) {
                let value=decode(child,"field");
                let value=if edge(child) {format!("Box::new({value})")} else {value};
                format!("{found}.map(|field|Ok::<_,String>({value})).transpose()?,")
            } else {format!("{},",decode(child,&format!("{found}.ok_or(\"prepared field missing\")?")))}
        }).collect::<Vec<_>>().concat();
        let entries=fields.iter().enumerate().map(|(i,(_,child))| {
            if optional.contains(&i) {
                let value=if edge(child) {"field.as_ref()"} else {"field"};
                format!("if let Some(field)=&value.{i} {{fields.push(({i},{}));}}",encode(child,value))
            } else { format!("fields.push(({i},{}));",encode(child,&format!("&value.{i}"))) }
        }).collect::<Vec<_>>().concat();
        let metadata=crate::checked_data::ty(ty);
        Some(format!(r#"
impl {name} {{
 fn decode_recursive(value:&hgl_semantics::ir::Value)->Result<Owned{name},String> {{
  if value.ty!={metadata} {{return Err("prepared recursive nominal mismatch".into());}}
  let hgl_semantics::ir::Kind::Construct(items)=&value.kind else {{return Err("prepared recursive value required".into())}};
  Ok(Owned{name}({values}))
 }}
 fn encode_recursive(value:&Owned{name})->hgl_semantics::ir::Value {{
  let mut fields=Vec::new(); {entries}
  hgl_semantics::ir::Value::new({metadata},hgl_semantics::ir::Kind::Construct(fields))
 }}
}}
"#))
    }).collect()
}
fn edge(ty: &Ty) -> bool {
    matches!(ty,Ty::Recursive(batch) if batch.definitions().is_empty())
}
