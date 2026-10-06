//! Generated concrete nominal ownership and field-presence operations.
use hgl_source::Ty;
/// Emit owning storage and exact field presence for one concrete checked struct.
pub fn marker(ty: &Ty, global_type: fn(&Ty) -> String, global_schema: fn(&Ty) -> String) -> String {
    let (identity, fields, optional) = ty
        .structure()
        .unwrap_or_else(|_| unreachable!("collected complete nominal roots"));
    let identity = identity.source_name();
    let name = global_type(ty);
    let (value, prefix, declaration) = representation(ty, global_type);
    let slots = tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
        format!(
            "hgl_store::ValueSlot<{}>",
            field_marker(ty, optional.contains(&i), global_type)
        )
    }));
    let schema = marker_schema(&identity, fields, optional, global_schema);
    let bind = tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
        format!(
            "hgl_store::ValueSlot::<{}>::bind(layout)",
            field_marker(ty, optional.contains(&i), global_type)
        )
    }));
    let retain = tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
        format!(
            "<{} as hgl_store::GlobalValue>::retain(&value.{i})?",
            field_marker(ty, optional.contains(&i), global_type)
        )
    }));
    let read = tuple(
        fields
            .iter()
            .enumerate()
            .map(|(i, _)| format!("slots.{i}.read(columns)?")),
    );
    let commit = fields
        .iter()
        .enumerate()
        .map(|(i, _)| format!("slots.{i}.commit(columns, value.{i}, layouts);"))
        .collect::<Vec<_>>()
        .concat();
    let (width, flatten) = positions(fields, optional, global_type);
    let prepare = fields
        .iter()
        .enumerate()
        .map(|(i, (_, ty))| {
            format!(
                "<{} as hgl_store::GlobalValue>::prepare(&value.{i}, capacity, layouts)?;",
                field_marker(ty, optional.contains(&i), global_type)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let install = tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
        format!(
            "hgl_store::ValueSlot::<{}>::install(columns,value.{i},layouts)",
            field_marker(ty, optional.contains(&i), global_type)
        )
    }));
    let release = fields
        .iter()
        .enumerate()
        .map(|(i, _)| format!("slots.{i}.release(columns);"))
        .collect::<Vec<_>>()
        .concat();

    format!(
        "{declaration}#[derive(Debug)]\nstruct {name};\nimpl hgl_store::GlobalValue for {name} {{\ntype Value = {value};\ntype Slots = {slots};\nconst WIDTH: usize = {width};\nfn prepare(value: &Self::Value, capacity: &mut hgl_store::Capacity, layouts: &mut hgl_store::Layouts) -> hgl_types::NodeResult {{ {prepare} Ok(()) }}\nfn install(columns: &mut hgl_store::ValueColumns, value: Self::Value, layouts: &mut hgl_store::Layouts) -> Self::Slots {{ {install} }}\nfn release(columns: &mut hgl_store::ValueColumns, slots: Self::Slots) {{ {release} }}\nfn flatten(slots: Self::Slots, layout: &mut [usize]) {{ {flatten} }}\nfn schema() -> hgl_types::OrdinaryType {{ {schema} }}\nfn slots(layout: &mut &[usize]) -> Self::Slots {{ {bind} }}\nfn retain(value: &Self::Value) -> Result<Self::Value, Box<hgl_types::NodeError>> {{ Ok({prefix}{retain}) }}\nfn read(columns: &hgl_store::ValueColumns, slots: Self::Slots) -> Result<Self::Value, Box<hgl_types::NodeError>> {{ Ok({prefix}{read}) }}\nfn commit(columns: &mut hgl_store::ValueColumns, slots: Self::Slots, value: Self::Value, layouts: &mut hgl_store::Layouts) {{ {commit} }}\n}}\n"
    ) + &hgl_rust_prepared_values::structure_fields(
        &name,
        &fields
            .iter()
            .enumerate()
            .map(|(i, (_, ty))| field_marker(ty, optional.contains(&i), global_type))
            .collect::<Vec<_>>(),
    ) + &recursive_equality(ty, global_type)
}

fn marker_schema(
    identity: &str,
    fields: &[(String, Ty)],
    optional: &[usize],
    global_schema: fn(&Ty) -> String,
) -> String {
    let positional = identity.starts_with("\0tuple<");
    let values = fields
        .iter()
        .enumerate()
        .map(|(i, (name, ty))| {
            let ty = if optional.contains(&i) {
                format!(
                    "hgl_types::OrdinaryType::OptionalField(Box::new({}))",
                    global_schema(ty)
                )
            } else {
                global_schema(ty)
            };
            if positional {
                ty
            } else {
                format!("({name:?}, {ty})")
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    if positional {
        format!("hgl_types::OrdinaryType::Tuple(vec![{values}])")
    } else {
        format!("hgl_types::OrdinaryType::Struct({identity:?}, vec![{values}])")
    }
}

fn field_marker(ty: &Ty, optional: bool, global_type: fn(&Ty) -> String) -> String {
    let marker = global_type(ty);
    let marker = if matches!(ty,Ty::Recursive(batch) if batch.definitions().is_empty()) {
        format!("hgl_store::Recursive<{marker}>")
    } else {
        marker
    };
    if optional {
        format!("hgl_store::Optional<{marker}>")
    } else {
        marker
    }
}

fn positions(
    fields: &[(String, Ty)],
    optional: &[usize],
    global_type: fn(&Ty) -> String,
) -> (String, String) {
    let widths = fields
        .iter()
        .enumerate()
        .map(|(i, (_, ty))| {
            format!(
                "<{} as hgl_store::GlobalValue>::WIDTH",
                field_marker(ty, optional.contains(&i), global_type)
            )
        })
        .collect::<Vec<_>>();
    let width = if widths.is_empty() {
        "0".into()
    } else {
        widths.join("+")
    };
    let flatten = widths.iter().enumerate().map(|(i,width)| format!("let (field, rest) = layout.split_at_mut({width}); slots.{i}.flatten(field); let layout = rest;")).collect::<Vec<_>>().concat();
    (width, flatten)
}
fn tuple(fields: impl Iterator<Item = String>) -> String {
    match fields.collect::<Vec<_>>().as_slice() {
        [] => "()".into(),
        fields => format!("({},)", fields.join(",")),
    }
}

fn representation(ty: &Ty, global_type: fn(&Ty) -> String) -> (String, String, String) {
    let (identity, fields, optional) = ty
        .structure()
        .unwrap_or_else(|_| unreachable!("complete nominal root"));
    let identity = identity.source_name();
    let name = global_type(ty);
    let value = tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
        format!(
            "<{} as hgl_store::GlobalValue>::Value",
            field_marker(ty, optional.contains(&i), global_type)
        )
    }));
    let recursive = matches!(ty, Ty::Recursive(_) | Ty::Family(_));
    let declaration = if recursive {
        format!(
            "#[derive(Debug,Clone,PartialEq)] struct Owned{name}{value};\nimpl hgl_store::RecursiveTarget for {name} {{ const IDENTITY:&'static str={identity:?}; }}\n"
        )
    } else {
        String::new()
    };
    let value = if recursive {
        format!("Owned{name}")
    } else {
        value
    };
    let prefix = if recursive {
        format!("Owned{name}")
    } else {
        String::new()
    };
    (value, prefix, declaration)
}

fn recursive_equality(ty: &Ty, marker: fn(&Ty) -> String) -> String {
    if !matches!(ty, Ty::Recursive(_)) {
        return String::new();
    }
    let (name, fields, optional) = ty
        .structure()
        .unwrap_or_else(|_| unreachable!("recursive root"));
    let schema = Ty::Struct(name.clone(), fields.to_vec(), optional.to_vec());
    let name = marker(ty);
    let comparison = hgl_rust_collections::equal(&schema, "left", "right", marker);
    format!(
        "impl {name} {{ fn ordinary_equal(left:&Owned{name},right:&Owned{name})->bool {{{comparison}}} }}"
    )
}
