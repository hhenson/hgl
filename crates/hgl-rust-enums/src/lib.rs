//! Prepared nominal enum layouts with statically selected i64 storage.
use hgl_source::EnumType;
/// Exact canonical enum marker name, independent of source import aliases.
pub fn marker_type(ty: &EnumType) -> String {
    format!(
        "GlobalEnum{}",
        ty.origin
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .concat()
    )
}
/// Emit a nominal enum marker delegating physical operations to typed i64 slots.
pub fn marker(ty: &EnumType) -> String {
    let name = marker_type(ty);
    let origin = &ty.origin;
    format!(
        r"
#[derive(Debug)] struct {name};
impl hgl_store::GlobalValue for {name} {{
    const PREPARED_SCALAR: bool = true;
    const WIDTH: usize = 1;
    type Value = i64;
    type Slots = usize;
    fn schema()->hgl_types::OrdinaryType {{hgl_types::OrdinaryType::Enum({origin:?})}}
    fn slots(layout:&mut &[usize])->usize {{<i64 as hgl_store::GlobalValue>::slots(layout)}}
    fn flatten(slots:usize,layout:&mut [usize]) {{<i64 as hgl_store::GlobalValue>::flatten(slots,layout)}}
    fn retain(value:&i64)->hgl_types::NodeResult<i64> {{Ok(*value)}}
    fn prepare(value:&i64,capacity:&mut hgl_store::Capacity,layouts:&mut hgl_store::Layouts)->hgl_types::NodeResult {{<i64 as hgl_store::GlobalValue>::prepare(value,capacity,layouts)}}
    fn read(columns:&hgl_store::ValueColumns,slots:usize)->hgl_types::NodeResult<i64> {{<i64 as hgl_store::GlobalValue>::read(columns,slots)}}
    fn commit(columns:&mut hgl_store::ValueColumns,slots:usize,value:i64,layouts:&mut hgl_store::Layouts) {{<i64 as hgl_store::GlobalValue>::commit(columns,slots,value,layouts)}}
    fn install(columns:&mut hgl_store::ValueColumns,value:i64,layouts:&mut hgl_store::Layouts)->usize {{<i64 as hgl_store::GlobalValue>::install(columns,value,layouts)}}
    fn release(columns:&mut hgl_store::ValueColumns,slots:usize) {{<i64 as hgl_store::GlobalValue>::release(columns,slots)}}
}}
impl hgl_store::Key for {name} {{
fn prepare(keys:&mut hgl_store::Keys,value:&i64)->hgl_types::NodeResult {{<i64 as hgl_store::Key>::prepare(keys,value)}}
fn id(keys:&hgl_store::Keys,value:&i64)->hgl_types::NodeResult<i64> {{<i64 as hgl_store::Key>::id(keys,value)}}
fn value(keys:&hgl_store::Keys,id:i64)->hgl_types::NodeResult<i64> {{<i64 as hgl_store::Key>::value(keys,id)}}
fn ids(keys:&hgl_store::Keys)->&[i64] {{<i64 as hgl_store::Key>::ids(keys)}}
}}

"
    )
}
