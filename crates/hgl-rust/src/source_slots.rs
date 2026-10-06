//! Typed projections into immutable node-owned ordinary configuration columns.
use crate::layouts::global_type;
use hgl_semantics::ir::{Kind, Value};
/// Return a typed source slot only when the complete path is immutable configuration.
/// Each checked index expression is evaluated once, in parent-before-index order.
pub fn projection(value: &Value, emit_index: impl Fn(&Value) -> String) -> Option<String> {
    project(value, &emit_index)
}
fn project(value: &Value, emit_index: &impl Fn(&Value) -> String) -> Option<String> {
    if let Kind::Configuration(id) = value.kind {
        return Some(format!("self.configuration_slot{id}"));
    }
    if let Kind::Field(parent, index) = &value.kind {
        return project(parent, emit_index).map(|slot| format!("({slot}).fields().{index}"));
    }
    if let Kind::Index(parent, index) = &value.kind {
        let parent = project(parent, emit_index)?;
        return Some(format!(
            "{{let parent={parent}; let index={}; hgl_store::ValueSlot::<{}>::bind(&mut hgl_store::list_index(self.configuration_columns.list(parent.fields()),index)?.as_slice())}}",
            emit_index(index),
            global_type(&value.ty)
        ));
    }
    None
}
/// Prepare independent typed configuration storage before any node hook executes.
/// Materialized configuration arguments are reused when `prepared` is true.
pub fn initialize(
    configurations: &[Value],
    prepared: bool,
    node_name: &str,
    emit: impl Fn(&Value) -> String,
) -> String {
    let mut code =
        vec!["let mut configuration_columns=hgl_store::ValueColumns::default();\n".into()];
    for (id, value) in configurations.iter().enumerate() {
        if !prepared {
            code.push(format!("let configuration{id}=(||->hgl_types::NodeResult<_>{{Ok({})}})().map_err(|e|hgl_describe::BuildError::InvalidNodeType {{node:{node_name:?},what:e.message}})?;",emit(value)));
        }
        let marker = global_type(&value.ty);
        code.push(format!("let configuration_slot{id}=(||->hgl_types::NodeResult<_>{{let mut bounds=<{marker} as hgl_store::PreparedValue>::Bounds::default();<{marker} as hgl_store::PreparedValue>::include(&mut bounds,&configuration{id});let slot=<{marker} as hgl_store::PreparedValue>::allocate(&mut configuration_columns,&bounds)?;<{marker} as hgl_store::PreparedValue>::check_native(&configuration_columns,slot,&configuration{id})?;<{marker} as hgl_store::PreparedValue>::copy_native(&mut configuration_columns,slot,&configuration{id});Ok(slot)}})().map_err(|e|hgl_describe::BuildError::InvalidNodeType {{node:{node_name:?},what:e.message}})?;"));
    }
    code.concat()
}
