use hgl_rust_layouts::global_type;
use hgl_source::Ty;
/// Private generated finite key-path storage; used only before graph startup.
pub const DECLARATION: &str = "#[derive(Default)] struct FiniteTopology {children:std::collections::BTreeMap<i64,FiniteTopology>}\n";
/// Merge one materialized exact structural recipe into its parent-specific key paths.
pub fn include(ty: &Ty, value: &str, domain: &str) -> String {
    if let Ty::Map(key, child) = ty {
        return format!(
            "for (key,value) in ({value}).0.iter().zip(&({value}).1) {{let key=<{} as hgl_store::Key>::id(&store.keys,key).map_err(|e|e.message)?;let domain=({domain}).children.entry(key).or_default();{}}}for key in &({value}).2 {{let key=<{} as hgl_store::Key>::id(&store.keys,key).map_err(|e|e.message)?;({domain}).children.entry(key).or_default();}}",
            global_type(key),
            include(child, "value", "domain"),
            global_type(key)
        );
    }
    if let Ty::Set(key) = ty {
        return format!(
            "for key in ({value}).0.iter().chain(&({value}).1) {{let key=<{} as hgl_store::Key>::id(&store.keys,key).map_err(|e|e.message)?;({domain}).children.entry(key).or_default();}}",
            global_type(key)
        );
    }
    if let Ty::List(child, _) = ty {
        return format!(
            "for (key,value) in ({value}).0.iter().zip(&({value}).1) {{let domain=({domain}).children.entry(*key).or_default();{}}}",
            include(child, "value", "domain")
        );
    }
    if let Ty::Struct(_, fields, _) = ty {
        return fields.iter().enumerate().map(|(i,(_,child))|format!("for value in &({value}).{i} {{let domain=({domain}).children.entry({i}).or_default();{}}}",include(child,"value","domain"))).collect::<Vec<_>>().concat();
    }
    if let Ty::Tuple(fields) = ty {
        return fields.iter().enumerate().map(|(i,child)|format!("for value in &({value}).{i} {{let domain=({domain}).children.entry({i}).or_default();{}}}",include(child,"value","domain"))).collect::<Vec<_>>().concat();
    }
    String::new()
}
