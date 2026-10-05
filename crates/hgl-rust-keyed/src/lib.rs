//! Static scalar-key collection operations with exclusively cold domain growth.
use hgl_rust_layouts::global_type;
use hgl_source::Ty;
mod prepare;
pub use prepare::{node_preparation, preparation};
fn push(target: &str, value: &str) -> String {
    format!(
        "{target}.try_reserve(1).map_err(|e|hgl_types::NodeError::new(e.to_string()))?;{target}.push({value});"
    )
}
/// Retain sparse ordinary keys by their statically selected exact type.
pub fn observation(key: &Ty, child: Option<&str>) -> String {
    let marker = global_type(key);
    let value = format!("<{marker} as hgl_store::Key>::value(&_ctx.store().keys,key)?");
    if let Some(child) = child {
        format!(
            "for &key in _ctx.store().bindings().changed_keys(input.id()) {{if let Some(child)=input.member(_ctx.store().bindings(),key) {{if !_ctx.store().input_valid(child.id()) {{return Err(hgl_types::NodeError::new(\"invalid structural child delta\"));}} let value={child}; {} {} }}}} for key in _ctx.store().bindings().removed_keys(input.id()) {{{}}}",
            push("delta.0", &value),
            push("delta.1", "value"),
            push("delta.2", &value)
        )
    } else {
        format!(
            "for key in _ctx.store().bindings().added_keys(input.id()) {{{}}} for key in _ctx.store().bindings().removed_keys(input.id()) {{{}}}",
            push("delta.0", &value),
            push("delta.1", &value)
        )
    }
}
/// Apply an exact typed key delta without extending the prepared domain.
pub fn application(key: &Ty, child: Option<(&str, &str)>) -> String {
    let marker = global_type(key);
    let id = format!("<{marker} as hgl_store::Key>::id(&_ctx.store().keys,&key)?");
    if let Some((allocate, apply)) = child {
        format!(
            "for key in &delta.2 {{let key={id};if output.member(_ctx.store().bindings(),key).is_none() {{return Err(hgl_types::NodeError::new(\"noncanonical map removal\"));}}}} for key in delta.2 {{let key={id};_ctx.remove_shaped(output.id(),key);}} for (key,value) in delta.0.into_iter().zip(delta.1) {{let key={id};_ctx.get_or_create_with(output.id(),key,|store,owner|{allocate});let child=output.member(_ctx.store().bindings(),key).ok_or_else(||hgl_types::NodeError::new(\"missing prepared map member\"))?;{apply}}}"
        )
    } else {
        format!(
            "for key in &delta.0 {{let key={id};if output.member(_ctx.store().bindings(),key).is_some() {{return Err(hgl_types::NodeError::new(\"noncanonical set addition\"));}}}} for key in &delta.1 {{let key={id};if output.member(_ctx.store().bindings(),key).is_none() {{return Err(hgl_types::NodeError::new(\"noncanonical set removal\"));}}}} for key in delta.1 {{let key={id};_ctx.remove_shaped(output.id(),key);}} for key in delta.0 {{let key={id};_ctx.get_or_create_with(output.id(),key,|store,owner|store.add_output::<bool>(owner).id());let child=output.member(_ctx.store().bindings(),key).ok_or_else(||hgl_types::NodeError::new(\"missing prepared set member\"))?;_ctx.set(hgl_store::Store::prepared_output(child),true);}}"
        )
    }
}
/// Prebuild an inactive finite keyed output before graph execution.
pub fn allocation(key: &Ty, shape: &str, child: &str) -> String {
    let marker = global_type(key);
    format!(
        "let keys=<{marker} as hgl_store::Key>::ids(&store.keys).to_vec();let output=store.add_prepared_output(owner,<{shape} as hgl_store::shapes::Shape>::shape(),Vec::new());store.prepare_collection(output,&keys,|store,owner|{{{child}}});output"
    )
}
