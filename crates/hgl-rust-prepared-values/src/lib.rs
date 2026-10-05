//! Typed prepared-copy implementations for generated nominal ordinary layouts.
use hgl_source::Ty;
use std::fmt::Write as _;
fn append(code: &mut String, text: std::fmt::Arguments<'_>) {
    code.write_fmt(text)
        .unwrap_or_else(|_| unreachable!("String formatting"));
}

/// Emit independent bounds and copies for the existing tuple-backed nominal marker.
pub fn structure(name: &str, fields: &[(String, Ty)], marker: impl Fn(&Ty) -> String) -> String {
    let mut bounds = String::new();
    let mut include = String::new();
    let mut allocated = Vec::new();
    let mut check_native = String::new();
    let mut check_slots = String::new();
    let mut copy_native = String::new();
    let mut copy_between = String::new();
    let mut copy_within = String::new();
    for (i, (_, ty)) in fields.iter().enumerate() {
        let child = format!("<{} as hgl_store::PreparedValue>", marker(ty));
        append(&mut bounds, format_args!("field{i}:{child}::Bounds,\n"));
        append(
            &mut include,
            format_args!("{child}::include(&mut bounds.field{i},&value.{i});"),
        );
        allocated.push(format!("{child}::allocate(columns,&bounds.field{i})?"));
        append(
            &mut check_native,
            format_args!("{child}::check_native(columns,destination.fields().{i},&value.{i})?;"),
        );
        append(
            &mut check_slots,
            format_args!(
                "{child}::check_slots(source,from.fields().{i},destination,to.fields().{i})?;"
            ),
        );
        append(
            &mut copy_native,
            format_args!("{child}::copy_native(columns,destination.fields().{i},&value.{i});"),
        );
        append(
            &mut copy_between,
            format_args!(
                "{child}::copy_between(source,from.fields().{i},destination,to.fields().{i});"
            ),
        );
        append(
            &mut copy_within,
            format_args!("{child}::copy_within(columns,from.fields().{i},to.fields().{i});"),
        );
    }
    let allocated = if allocated.is_empty() {
        "()".into()
    } else {
        format!("({},)", allocated.join(","))
    };
    format!(
        r"
#[derive(Default)] struct PreparedBounds{name} {{ {bounds} }}
impl hgl_store::PreparedValue for {name} {{
 type Bounds=PreparedBounds{name};
 fn include(bounds:&mut Self::Bounds,value:&Self::Value) {{ {include} }}
 fn allocate(columns:&mut hgl_store::ValueColumns,bounds:&Self::Bounds)->hgl_types::NodeResult<hgl_store::ValueSlot<Self>> {{Ok(hgl_store::ValueSlot::from_fields({allocated}))}}
 fn check_native(columns:&hgl_store::ValueColumns,destination:hgl_store::ValueSlot<Self>,value:&Self::Value)->hgl_types::NodeResult {{ {check_native} Ok(()) }}
 fn check_slots(source:&hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,destination:&hgl_store::ValueColumns,to:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult {{ {check_slots} Ok(()) }}
 fn copy_native(columns:&mut hgl_store::ValueColumns,destination:hgl_store::ValueSlot<Self>,value:&Self::Value) {{ {copy_native} }}
 fn copy_between(source:&hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,destination:&mut hgl_store::ValueColumns,to:hgl_store::ValueSlot<Self>) {{ {copy_between} }}
 fn copy_within(columns:&mut hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,to:hgl_store::ValueSlot<Self>) {{ {copy_within} }}
}}
"
    )
}
/// Reuse the i64 physical copy operations while keeping the nominal marker.
pub fn enumeration(name: &str) -> String {
    format!(
        r"
impl hgl_store::PreparedValue for {name} {{
 type Bounds=<i64 as hgl_store::PreparedValue>::Bounds;
 fn include(bounds:&mut Self::Bounds,value:&Self::Value) {{ <i64 as hgl_store::PreparedValue>::include(bounds,value); }}
 fn allocate(columns:&mut hgl_store::ValueColumns,bounds:&Self::Bounds)->hgl_types::NodeResult<hgl_store::ValueSlot<Self>> {{Ok(hgl_store::ValueSlot::from_fields(<i64 as hgl_store::PreparedValue>::allocate(columns,bounds)?.fields()))}}
 fn check_native(columns:&hgl_store::ValueColumns,destination:hgl_store::ValueSlot<Self>,value:&Self::Value)->hgl_types::NodeResult {{ <i64 as hgl_store::PreparedValue>::check_native(columns,hgl_store::ValueSlot::from_fields(destination.fields()),value) }}
 fn check_slots(source:&hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,destination:&hgl_store::ValueColumns,to:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult {{ <i64 as hgl_store::PreparedValue>::check_slots(source,hgl_store::ValueSlot::from_fields(from.fields()),destination,hgl_store::ValueSlot::from_fields(to.fields())) }}
 fn copy_native(columns:&mut hgl_store::ValueColumns,destination:hgl_store::ValueSlot<Self>,value:&Self::Value) {{ <i64 as hgl_store::PreparedValue>::copy_native(columns,hgl_store::ValueSlot::from_fields(destination.fields()),value); }}
 fn copy_between(source:&hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,destination:&mut hgl_store::ValueColumns,to:hgl_store::ValueSlot<Self>) {{ <i64 as hgl_store::PreparedValue>::copy_between(source,hgl_store::ValueSlot::from_fields(from.fields()),destination,hgl_store::ValueSlot::from_fields(to.fields())); }}
 fn copy_within(columns:&mut hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,to:hgl_store::ValueSlot<Self>) {{ <i64 as hgl_store::PreparedValue>::copy_within(columns,hgl_store::ValueSlot::from_fields(from.fields()),hgl_store::ValueSlot::from_fields(to.fields())); }}
}}
"
    )
}
