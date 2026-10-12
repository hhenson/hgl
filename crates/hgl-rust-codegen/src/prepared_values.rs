//! Typed prepared-copy implementations for generated nominal ordinary layouts.
use crate::append;
use hgl_source::Ty;

/// Emit independent bounds and copies for the existing tuple-backed nominal marker.
pub fn structure(name: &str, fields: &[(String, Ty)], marker: impl Fn(&Ty) -> String) -> String {
    structure_fields(
        name,
        &fields.iter().map(|(_, ty)| marker(ty)).collect::<Vec<_>>(),
    )
}
/// Emit prepared operations from already selected per-field storage markers.
pub fn structure_fields(name: &str, markers: &[String]) -> String {
    let mut bounds = String::new();
    let mut allocated = Vec::new();
    let mut methods: [String; 6] = std::array::from_fn(|_| String::new());
    for (i, marker) in markers.iter().enumerate() {
        let child = format!("<{marker} as hgl_store::PreparedValue>");
        append(&mut bounds, format_args!("field{i}:{child}::Bounds,\n"));
        allocated.push(format!("{child}::allocate(columns,&bounds.field{i})?"));
        let calls = [
            format!("include(&mut bounds.field{i},&value.{i});"),
            format!("check_native(columns,destination.fields().{i},&value.{i})?;"),
            format!("check_slots(source,from.fields().{i},destination,to.fields().{i})?;"),
            format!("copy_native(columns,destination.fields().{i},&value.{i});"),
            format!("copy_between(source,from.fields().{i},destination,to.fields().{i});"),
            format!("copy_within(columns,from.fields().{i},to.fields().{i});"),
        ];
        for (method, call) in methods.iter_mut().zip(calls) {
            append(method, format_args!("{child}::{call}"));
        }
    }
    let [
        include,
        check_native,
        check_slots,
        copy_native,
        copy_between,
        copy_within,
    ] = methods;
    let allocated = if allocated.is_empty() {
        "()".into()
    } else {
        format!("({},)", allocated.join(","))
    };
    format!(
        r"
#[derive(Default,Clone)] struct PreparedBounds{name} {{ {bounds} }}
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
    delegate(name, "i64")
}
/// Reuse a compatible physical marker while preserving exact source schema.
pub fn delegate(name: &str, marker: &str) -> String {
    format!(
        r"
impl hgl_store::PreparedValue for {name} {{
 type Bounds=<{marker} as hgl_store::PreparedValue>::Bounds;
 fn include(bounds:&mut Self::Bounds,value:&Self::Value) {{ <{marker} as hgl_store::PreparedValue>::include(bounds,value); }}
 fn allocate(columns:&mut hgl_store::ValueColumns,bounds:&Self::Bounds)->hgl_types::NodeResult<hgl_store::ValueSlot<Self>> {{Ok(hgl_store::ValueSlot::from_fields(<{marker} as hgl_store::PreparedValue>::allocate(columns,bounds)?.fields()))}}
 fn check_native(columns:&hgl_store::ValueColumns,destination:hgl_store::ValueSlot<Self>,value:&Self::Value)->hgl_types::NodeResult {{ <{marker} as hgl_store::PreparedValue>::check_native(columns,hgl_store::ValueSlot::from_fields(destination.fields()),value) }}
 fn check_slots(source:&hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,destination:&hgl_store::ValueColumns,to:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult {{ <{marker} as hgl_store::PreparedValue>::check_slots(source,hgl_store::ValueSlot::from_fields(from.fields()),destination,hgl_store::ValueSlot::from_fields(to.fields())) }}
 fn copy_native(columns:&mut hgl_store::ValueColumns,destination:hgl_store::ValueSlot<Self>,value:&Self::Value) {{ <{marker} as hgl_store::PreparedValue>::copy_native(columns,hgl_store::ValueSlot::from_fields(destination.fields()),value); }}
 fn copy_between(source:&hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,destination:&mut hgl_store::ValueColumns,to:hgl_store::ValueSlot<Self>) {{ <{marker} as hgl_store::PreparedValue>::copy_between(source,hgl_store::ValueSlot::from_fields(from.fields()),destination,hgl_store::ValueSlot::from_fields(to.fields())); }}
 fn copy_within(columns:&mut hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,to:hgl_store::ValueSlot<Self>) {{ <{marker} as hgl_store::PreparedValue>::copy_within(columns,hgl_store::ValueSlot::from_fields(from.fields()),hgl_store::ValueSlot::from_fields(to.fields())); }}
}}
"
    )
}
