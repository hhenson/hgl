//! Exact composite key identities and prepared transport without owning intermediaries.
use crate::append;
use hgl_source::Ty;

/// Whether an ordinary marker needs complete composite key operations.
pub fn composite(ty: &Ty) -> bool {
    matches!(ty, Ty::Tuple(_) | Ty::Struct(..)) && ty.collection_key()
}
/// Resolve an exact key directly from prepared typed field storage.
pub fn slot_id(ty: &Ty, slot: &str, marker: fn(&Ty) -> String) -> String {
    format!("{}?", slot_operation(ty, slot, marker, "id"))
}
fn slot_operation(ty: &Ty, slot: &str, marker: fn(&Ty) -> String, operation: &str) -> String {
    let name = marker(ty);
    if matches!(ty, Ty::Native(_)) {
        return format!("<{name}>::key_view_{operation}(keys,source.native({slot}))");
    }
    if *ty == Ty::Any {
        return format!(
            "<hgl_store::Any as hgl_store::Key>::{operation}(keys,source.any(({slot}).fields()))"
        );
    }
    if composite(ty) {
        format!("{name}::key_slot_{operation}(keys,source,{slot})")
    } else {
        let scalar = if matches!(ty, Ty::Enum(_)) {
            "i64".into()
        } else {
            name.clone()
        };
        format!(
            "<{name} as hgl_store::Key>::{operation}(keys,source.scalar::<{scalar}>(({slot}).fields()))"
        )
    }
}
/// Resolve optional membership from borrowed typed key storage, preserving failures.
pub fn slot_find(ty: &Ty, slot: &str, marker: fn(&Ty) -> String) -> String {
    slot_operation(ty, slot, marker, "find")
}
fn found(expression: &str) -> String {
    format!("match ({expression})? {{Some(id)=>id,None=>{{complete=false;0}}}}")
}
/// Copy a key's retained components directly into an already reserved typed slot.
pub fn copy(ty: &Ty, id: &str, slot: &str, marker: fn(&Ty) -> String) -> String {
    let name = marker(ty);
    if matches!(ty, Ty::Native(_)) {
        return format!(
            "{{let value=<{name}>::key_view(keys,{id})?;<{name}>::check_view(columns,{slot},value)?;<{name}>::copy_view(columns,{slot},value);}}"
        );
    }
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
    let (find_ids, find_slots) = find_components(fields, optional, marker);
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
fn find(keys:&hgl_store::Keys,value:&Self::Value)->hgl_types::NodeResult<Option<i64>> {{let mut complete=true;let parts=[{find_ids}];Ok(if complete {{keys.find_composite({domain},&parts)}} else {{None}})}}
fn value(keys:&hgl_store::Keys,id:i64)->hgl_types::NodeResult<Self::Value> {{let parts=keys.composite_parts({domain},id)?;Ok({values})}}
fn ids(keys:&hgl_store::Keys)->&[i64] {{keys.composite_ids({domain})}}
}}
impl {name} {{
fn key_parts_find(keys:&hgl_store::Keys,parts:&[i64])->Option<i64> {{keys.find_composite({domain},parts)}}
fn key_slot_id(keys:&hgl_store::Keys,source:&hgl_store::ValueColumns,slot:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult<i64> {{keys.composite_id({domain},&[{slots}])}}
fn key_slot_find(keys:&hgl_store::Keys,source:&hgl_store::ValueColumns,slot:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult<Option<i64>> {{let mut complete=true;let parts=[{find_slots}];Ok(if complete {{keys.find_composite({domain},&parts)}} else {{None}})}}
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

fn find_components(
    fields: &[(String, Ty)],
    optional: &[usize],
    marker: fn(&Ty) -> String,
) -> (String, String) {
    let find_ids = fields.iter().enumerate().map(|(i, (_, child))| {
        let value = found(&format!("<{} as hgl_store::Key>::find(keys,value)", marker(child)));
        if optional.contains(&i) {
            format!("i64::from(value.{i}.is_some()),match &value.{i} {{Some(value)=>{value},None=>0}}")
        } else {
            format!("1,{{let value=&value.{i};{value}}}")
        }
    }).collect::<Vec<_>>().join(",");
    let find_slots = fields.iter().enumerate().map(|(i, (_, child))| {
        let field = format!("slot.fields().{i}");
        if optional.contains(&i) {
            let child_slot = optional_slot(&marker(child), &field, "source", false);
            format!("i64::from(!source.list({field}.fields()).is_empty()),if source.list({field}.fields()).is_empty() {{0}} else {{{}}}", found(&slot_find(child, &child_slot, marker)))
        } else {
            format!("1,{}", found(&slot_find(child, &field, marker)))
        }
    }).collect::<Vec<_>>().join(",");
    (find_ids, find_slots)
}

/// Resolve a structural temporal key from statically typed child inputs.
pub fn input_find(ty: &Ty, input: &str, marker: fn(&Ty) -> String) -> String {
    let fields = if let Ty::Struct(_, fields, optional) = ty {
        fields
            .iter()
            .enumerate()
            .map(|(i, (_, child))| (i, child, optional.contains(&i)))
            .collect::<Vec<_>>()
    } else if let Ty::Tuple(fields) = ty {
        fields
            .iter()
            .enumerate()
            .map(|(i, child)| (i, child, false))
            .collect()
    } else {
        let name = marker(ty);
        let read = if matches!(ty, Ty::Native(_)) {
            return format!(
                "<{name}>::key_view_find(keys,_ctx.store().atomic_borrow::<{name}>({input})?)"
            );
        } else if matches!(ty, Ty::Any | Ty::Enum(_)) {
            format!("&_ctx.store().atomic_borrow::<{name}>({input})?")
        } else {
            format!("_ctx.store().get_ref(hgl_store::Store::prepared_input({input}))")
        };
        return format!("<{name} as hgl_store::Key>::find(keys,{read})");
    };
    let parts = fields.into_iter().map(|(i, child, optional)| {
        let input = format!("({input}).field::<{i}>(_ctx.store().bindings())");
        let id = found(&input_find(child, &input, marker));
        if optional {
            format!("i64::from(_ctx.store().input_valid(({input}).id())),if _ctx.store().input_valid(({input}).id()) {{{id}}} else {{0}}")
        } else {
            format!("1,{id}")
        }
    }).collect::<Vec<_>>().join(",");
    format!(
        "{{let mut complete=true;let parts=[{parts}];Ok::<_,hgl_types::NodeError>(if complete {{{}::key_parts_find(keys,&parts)}} else {{None}})}}",
        marker(ty)
    )
}
