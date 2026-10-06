//! Exact composite key identities and prepared transport without owning intermediaries.
use hgl_source::Ty;
use std::fmt::Write as _;
fn append(out: &mut String, args: std::fmt::Arguments<'_>) {
    out.write_fmt(args)
        .unwrap_or_else(|_| unreachable!("String formatting"));
}
/// Whether an ordinary marker needs complete composite key operations.
pub fn composite(ty: &Ty) -> bool {
    matches!(ty, Ty::Tuple(_) | Ty::Struct(..)) && ty.collection_key()
}
/// Resolve an exact key directly from prepared typed field storage.
pub fn slot_id(ty: &Ty, slot: &str, marker: fn(&Ty) -> String) -> String {
    let name = marker(ty);
    if composite(ty) {
        format!("{name}::key_slot_id(keys,source,{slot})?")
    } else {
        let scalar = if matches!(ty, Ty::Enum(_)) {
            "i64".into()
        } else {
            name.clone()
        };
        format!(
            "<{name} as hgl_store::Key>::id(keys,source.scalar::<{scalar}>(({slot}).fields()))?"
        )
    }
}
/// Copy a key's retained components directly into an already reserved typed slot.
pub fn copy(ty: &Ty, id: &str, slot: &str, marker: fn(&Ty) -> String) -> String {
    let name = marker(ty);
    if composite(ty) {
        format!("{name}::copy_key(keys,{id},columns,{slot})?;")
    } else {
        format!(
            "<{name} as hgl_store::Key>::with_value(keys,{id},|value|->hgl_types::NodeResult {{<{name} as hgl_store::PreparedValue>::check_native(columns,{slot},value)?;<{name} as hgl_store::PreparedValue>::copy_native(columns,{slot},value);Ok(())}})??;"
        )
    }
}
/// Emit one marker's statically numbered domain and exact native/slot operations.
pub fn implementation(ty: &Ty, domain: usize, marker: fn(&Ty) -> String) -> String {
    let Ty::Struct(_, fields, optional) = ty else {
        unreachable!("collected tuple/struct storage")
    };
    let name = marker(ty);
    let mut prepare = String::new();
    let mut ids = Vec::new();
    let mut values = Vec::new();
    let mut slots = Vec::new();
    let mut copies = String::new();
    for (i, (_, child)) in fields.iter().enumerate() {
        let child_marker = marker(child);
        let offset = i * 2;
        let index = offset + 1;
        let field = format!("slot.fields().{i}");
        let child_id = format!("<{child_marker} as hgl_store::Key>::id(keys,value)?");
        let value = format!("<{child_marker} as hgl_store::Key>::value(keys,parts[{index}])?");
        let seed = format!("<{child_marker} as hgl_store::Key>::prepare(keys,value)?;");
        if optional.contains(&i) {
            append(
                &mut prepare,
                format_args!("if let Some(value)=&value.{i} {{{seed}}}"),
            );
            ids.push(format!("i64::from(value.{i}.is_some()),match &value.{i} {{Some(value)=>{child_id},None=>0}}"));
            values.push(format!(
                "if parts[{offset}]==0 {{None}} else {{Some({value})}}"
            ));
            let child_slot = optional_slot(&child_marker, &field, "source", false);
            slots.push(format!("i64::from(!source.list({field}.fields()).is_empty()),if source.list({field}.fields()).is_empty() {{0}} else {{{}}}", slot_id(child, &child_slot, marker)));
            let child_slot = optional_slot(&child_marker, &field, "columns", true);
            append(
                &mut copies,
                format_args!(
                    "if parts[{offset}]==0 {{columns.set_list_len({field}.fields(),0);}} else {{let destination={child_slot};{}columns.set_list_len({field}.fields(),1);}}",
                    copy(child, &format!("parts[{index}]"), "destination", marker)
                ),
            );
        } else {
            append(
                &mut prepare,
                format_args!("{{let value=&value.{i};{seed}}}"),
            );
            ids.push(format!("1,{{let value=&value.{i};{child_id}}}"));
            values.push(value);
            slots.push(format!("1,{}", slot_id(child, &field, marker)));
            copies += &copy(child, &format!("parts[{index}]"), &field, marker);
        }
    }
    let ids = ids.join(",");
    let slots = slots.join(",");
    let values = if values.is_empty() {
        "()".into()
    } else {
        format!("({},)", values.join(","))
    };
    format!(
        r"
impl hgl_store::Key for {name} {{
fn prepare(keys:&mut hgl_store::Keys,value:&Self::Value)->hgl_types::NodeResult {{{prepare}keys.prepare_composite({domain},&[{ids}])}}
fn id(keys:&hgl_store::Keys,value:&Self::Value)->hgl_types::NodeResult<i64> {{keys.composite_id({domain},&[{ids}])}}
fn value(keys:&hgl_store::Keys,id:i64)->hgl_types::NodeResult<Self::Value> {{let parts=keys.composite_parts({domain},id)?;Ok({values})}}
fn ids(keys:&hgl_store::Keys)->&[i64] {{keys.composite_ids({domain})}}
}}
impl {name} {{
fn key_slot_id(keys:&hgl_store::Keys,source:&hgl_store::ValueColumns,slot:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult<i64> {{keys.composite_id({domain},&[{slots}])}}
fn copy_key(keys:&hgl_store::Keys,id:i64,columns:&mut hgl_store::ValueColumns,slot:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult {{let parts=keys.composite_parts({domain},id)?;{copies}Ok(())}}
}}
"
    )
}
fn optional_slot(marker: &str, field: &str, columns: &str, prepared: bool) -> String {
    let list = if prepared { "prepared_list" } else { "list" };
    format!(
        "hgl_store::ValueSlot::<{marker}>::bind(&mut {columns}.{list}({field}.fields()).first().ok_or_else(||hgl_types::NodeError::new(\"optional key field was not prepared\"))?.as_slice())"
    )
}
