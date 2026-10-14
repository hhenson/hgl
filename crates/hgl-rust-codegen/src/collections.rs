//! Exact ordinary collection layout and ordered constructor emission.
use crate::append;
use hgl_semantics::ir::{Kind, Value};
use hgl_source::{FamilyType, Ty};

/// Homogeneous retained element; maps retain complete typed key/value pairs.
pub fn element(ty: &Ty) -> Option<Ty> {
    if let Ty::List(child, _) | Ty::Set(child) = ty {
        return Some((**child).clone());
    }
    if let Ty::Map(key, child) = ty {
        return Some(Ty::Tuple(vec![(**key).clone(), (**child).clone()]));
    }
    None
}
/// Emit an exact schema marker delegating physical retention to typed list storage.
pub fn marker(ty: &Ty, global: fn(&Ty) -> String, schema: fn(&Ty) -> String) -> String {
    let name = global(ty);
    let item = element(ty).unwrap_or_else(|| unreachable!("checked collection"));
    let base = format!("hgl_store::List<{}>", global(&item));
    let schema = if let Ty::Set(key) = ty {
        format!("hgl_types::OrdinaryType::Set(Box::new({}))", schema(key))
    } else if let Ty::Map(key, value) = ty {
        format!(
            "hgl_types::OrdinaryType::Map(Box::new({}),Box::new({}))",
            schema(key),
            schema(value)
        )
    } else {
        unreachable!("checked collection marker")
    };
    let map = matches!(ty, Ty::Map(..));
    let value_type = if map {
        format!(
            "hgl_store::map_values::MapValue<{}>",
            crate::layouts::owned_type(&item)
        )
    } else {
        format!("<{base} as hgl_store::GlobalValue>::Value")
    };
    let native = if map { "value.entries()" } else { "value" };
    let retain = if map {
        format!(
            "Ok(hgl_store::map_values::MapValue::from_parts(<{base} as hgl_store::GlobalValue>::retain(value.entries())?,value.index().retained()?))"
        )
    } else {
        format!("<{base} as hgl_store::GlobalValue>::retain(value)")
    };
    let read = if map {
        format!(
            "Ok(hgl_store::map_values::MapValue::from_parts(<{base} as hgl_store::GlobalValue>::read(columns,slot)?,columns.map_index(slot).retained()?))"
        )
    } else {
        format!("<{base} as hgl_store::GlobalValue>::read(columns,slot)")
    };
    let install = if map {
        format!(
            "let(value,index)=value.into_parts();let slot=<{base} as hgl_store::GlobalValue>::install(columns,value,layouts);columns.replace_map_index(slot,index);slot"
        )
    } else {
        format!("<{base} as hgl_store::GlobalValue>::install(columns,value,layouts)")
    };
    let commit = if map {
        format!(
            "let(value,index)=value.into_parts();<{base} as hgl_store::GlobalValue>::commit(columns,slot,value,layouts);columns.replace_map_index(slot,index);"
        )
    } else {
        format!("<{base} as hgl_store::GlobalValue>::commit(columns,slot,value,layouts);")
    };
    let prepared = if map {
        map_prepared(&name, &base)
    } else {
        crate::prepared_values::delegate(&name, &base)
    };
    format!(
        r"
#[derive(Debug)] struct {name};
impl hgl_store::GlobalValue for {name} {{
 type Value={value_type};
 type Slots=usize;
 const WIDTH:usize=1;
 fn schema()->hgl_types::OrdinaryType {{{schema}}}
 fn slots(layout:&mut &[usize])->usize {{<{base} as hgl_store::GlobalValue>::slots(layout)}}
 fn retain(value:&Self::Value)->hgl_types::NodeResult<Self::Value> {{{retain}}}
 fn prepare(value:&Self::Value,capacity:&mut hgl_store::Capacity,layouts:&mut hgl_store::Layouts)->hgl_types::NodeResult {{<{base} as hgl_store::GlobalValue>::prepare({native},capacity,layouts)}}
 fn install(columns:&mut hgl_store::ValueColumns,value:Self::Value,layouts:&mut hgl_store::Layouts)->usize {{{install}}}
 fn release(columns:&mut hgl_store::ValueColumns,slot:usize) {{<{base} as hgl_store::GlobalValue>::release(columns,slot);}}
 fn flatten(slot:usize,layout:&mut [usize]) {{<{base} as hgl_store::GlobalValue>::flatten(slot,layout);}}
 fn read(columns:&hgl_store::ValueColumns,slot:usize)->hgl_types::NodeResult<Self::Value> {{{read}}}
 fn commit(columns:&mut hgl_store::ValueColumns,slot:usize,value:Self::Value,layouts:&mut hgl_store::Layouts) {{{commit}}}
}}
"
    ) + &prepared
}
/// Emit complete construction, checking each retained key before its value runs.
pub fn construct(
    ty: &Ty,
    items: &[Value],
    emit: impl Fn(&Value) -> String,
    owned: fn(&Ty) -> String,
) -> String {
    let item = element(ty).unwrap_or_else(|| unreachable!("checked collection"));
    let mut code = format!("{{let mut items:Vec<{}>=Vec::new();", owned(&item));
    if matches!(ty, Ty::Map(..)) {
        code.push_str("let mut hashes=Vec::new();");
    }
    for value in items {
        let (key, child) = if matches!(ty, Ty::Map(..)) {
            let Kind::Construct(fields) = &value.kind else {
                unreachable!("checked map entry")
            };
            (&fields[0].1, Some(&fields[1].1))
        } else {
            (value, None)
        };
        append(&mut code, format_args!("let key={};", emit(key)));
        code.push_str(&key_checks(&key.ty, "key"));
        append(
            &mut code,
            format_args!(
                "{}{};",
                if child.is_some() { "let hash=" } else { "" },
                key_hash(&key.ty, "key")
            ),
        );
        let field = if child.is_some() { "&item.0" } else { "item" };
        append(
            &mut code,
            format_args!(
                "if items.iter().try_fold(false,|found,item|Ok::<bool,Box<hgl_types::NodeError>>(found||{}))? {{return Err(hgl_types::NodeError::new(\"duplicate ordinary collection key or member\"));}}",
                equal(&key.ty, field, "&key", crate::layouts::global_type)
            ),
        );
        if let Some(child) = child {
            append(
                &mut code,
                format_args!(
                    "let value={};items.push((key,value));hashes.push(hash);",
                    emit(child)
                ),
            );
        } else {
            code.push_str("items.push(key);");
        }
    }
    if matches!(ty, Ty::Map(..)) {
        code + "hgl_store::map_values::MapValue::from_parts(items,hgl_store::map_values::Index::from_hashes(hashes)?) }"
    } else {
        code + "items}"
    }
}
/// Recursive native equality with unordered complete set and map children.
pub fn equal(ty: &Ty, a: &str, b: &str, marker: fn(&Ty) -> String) -> String {
    if let Ty::Result(ok, err) = ty {
        return format!(
            "match (({a}).as_ref(),({b}).as_ref()) {{(Ok(left),Ok(right))=>{},(Err(left),Err(right))=>{},_=>false}}",
            equal(ok, "left", "right", marker),
            equal(err, "left", "right", marker)
        );
    }
    if *ty == Ty::Any {
        return format!("({a}).equal({b})?");
    }
    if matches!(ty, Ty::Native(_)) {
        return format!("({a}).view().equal(({b}).view())?");
    }
    if matches!(ty, Ty::Recursive(_)) {
        return format!("{}::ordinary_equal({a},{b})?", marker(ty));
    }
    if let Ty::Family(family) = ty {
        return equal(&crate::families::family_storage(family), a, b, marker);
    }
    if let Some(item) = element(ty) {
        let compare = equal(&item, "left", "right", marker);
        return if matches!(ty, Ty::Set(_) | Ty::Map(..)) {
            format!(
                "({a}).len()==({b}).len() && ({a}).iter().try_fold(true,|same,left|Ok::<bool,Box<hgl_types::NodeError>>(same&&({b}).iter().try_fold(false,|found,right|Ok::<bool,Box<hgl_types::NodeError>>(found||{compare}))?))?"
            )
        } else {
            format!(
                "({a}).len()==({b}).len() && ({a}).iter().zip(({b}).iter()).try_fold(true,|same,(left,right)|Ok::<bool,Box<hgl_types::NodeError>>(same&&{compare}))?"
            )
        };
    }
    let Some((fields, optional)) = matches!(ty, Ty::Tuple(_) | Ty::Struct(..))
        .then(|| hgl_source::ordinary_shape::fields(ty))
        .flatten()
    else {
        return format!("({a})==({b})");
    };
    let checks = fields.iter().enumerate().map(|(i, t)| {
        let (left,right)=(format!("&({a}).{i}"),format!("&({b}).{i}"));
        if optional.contains(&i) {
            format!("match (({left}).as_ref(),({right}).as_ref()) {{(Some(left),Some(right))=>{},(None,None)=>true,_=>false}}",equal(t,"left","right",marker))
        } else {equal(t,&left,&right,marker)}
    }).collect::<Vec<_>>();
    if checks.is_empty() {
        "true".into()
    } else {
        checks.join(" && ")
    }
}

/// Emit an independent owning read using the exact source layout.
pub fn retained(source: &str, ty: &Ty, global_type: fn(&Ty) -> String) -> String {
    if let Ty::Nullable(child) = ty {
        return format!(
            "({source}).as_ref().map(|value|Ok::<_,Box<hgl_types::NodeError>>({})).transpose()?",
            retained("*value", child, global_type)
        );
    }
    if hgl_semantics::value_access::aggregate(ty)
        || matches!(ty, Ty::Native(_) | Ty::Result(..) | Ty::Error)
    {
        return format!(
            "<{} as hgl_store::GlobalValue>::retain(&({source}))?",
            global_type(ty)
        );
    }
    if (ty.owning_scalar() && *ty != Ty::Any) || matches!(ty, Ty::Enum(_)) {
        format!("hgl_store::Scalar::try_clone(&({source}))?")
    } else {
        format!("({source})")
    }
}

fn key_checks(ty: &Ty, value: &str) -> String {
    if *ty == Ty::F64 {
        return format!(
            "if ({value}).is_nan() {{return Err(hgl_types::NodeError::new(\"NaN collection key\"));}}"
        );
    }
    let Some((fields, optional)) = matches!(ty, Ty::Tuple(_) | Ty::Struct(..))
        .then(|| hgl_source::ordinary_shape::fields(ty))
        .flatten()
    else {
        return String::new();
    };
    fields
        .iter()
        .enumerate()
        .map(|(i, ty)| {
            let field = format!("({value}).{i}");
            if optional.contains(&i) {
                format!("if let Some(key)=&{field} {{{}}}", key_checks(ty, "key"))
            } else {
                key_checks(ty, &field)
            }
        })
        .collect()
}

/// Project an inherited family field through its exact retained member tag.
/// # Panics
/// Panics when supplied IR lacks the checked common field or concrete members.
pub fn family_field(family: &FamilyType, index: usize, source: &str, slot: bool) -> String {
    let (_, fields, _) = family.members()[0]
        .1
        .structure()
        .unwrap_or_else(|_| unreachable!("checked family member"));
    let name = &fields[index].0;
    let mut arms = String::new();
    for (tag, (_, ty)) in family.members().iter().enumerate() {
        let (_, fields, _) = ty
            .structure()
            .unwrap_or_else(|_| unreachable!("checked family member"));
        let index = fields
            .iter()
            .position(|(field, _)| field == name)
            .unwrap_or_else(|| unreachable!("checked common family field"));
        let field = if slot {
            format!(
                "hgl_store::ValueSlot::<{}>::bind(&mut columns.list(parent.fields().{}.fields()).first().expect(\"family member\").as_slice()).fields().{index}",
                crate::layouts::global_type(ty),
                tag + 1
            )
        } else {
            format!(
                "&parent.{}.as_ref().expect(\"family member\").{index}",
                tag + 1
            )
        };
        append(&mut arms, format_args!("{tag}=>{field},"));
    }
    if slot {
        return format!(
            "{{let parent={source};let columns=_ctx.store().global_values();match parent.fields().0.read(columns)? {{{arms}_=>unreachable!(\"checked family tag\")}}}}"
        );
    }
    format!(
        "*{{let parent=&({source});match parent.0 {{{arms}_=>unreachable!(\"checked family tag\")}}}}"
    )
}

/// Start a borrowed live or modified membership traversal without collecting keys.
pub fn iteration_start(
    shape: &Ty,
    input: &str,
    ids: (usize, usize),
    modified: bool,
    marker: fn(&Ty) -> String,
) -> String {
    let (key_id, child_id) = ids;
    if let Ty::List(_, Some(length)) = shape {
        let selected = if modified {
            format!("_ctx.store().bindings().modified(local{child_id}.id(),_ctx.evaluation_time())")
        } else {
            "true".into()
        };
        return format!(
            "let mut index{key_id}=0;while index{key_id}<{length} {{let local{key_id}=index{key_id} as i64;let local{child_id}={input}.index(_ctx.store().bindings(),index{key_id});index{key_id}+=1;if {selected} {{"
        );
    }
    let key = if let Ty::Map(key, _) = shape {
        key.as_ref()
    } else {
        &Ty::I64
    };
    let next = if modified {
        format!("_ctx.store().bindings().changed_keys({input}.id()).get(index{key_id}).copied()")
    } else {
        format!(
            "{{let table=&_ctx.store().bindings().input({input}.id()).members.live;if table.prepared() {{table.item(index{key_id}).map(|(key,_)|key)}} else {{table.dynamic_after(previous{key_id}).map(|(key,_)|key)}}}}"
        )
    };
    format!(
        "let mut index{key_id}=0;let mut previous{key_id}=None;while let Some(key{key_id})={next} {{index{key_id}+=1;previous{key_id}=Some(key{key_id});if let Some(local{child_id})={input}.member(_ctx.store().bindings(),key{key_id}) {{let local{key_id}=<{} as hgl_store::Key>::value(&_ctx.store().keys,key{key_id})?;",
        marker(key)
    )
}

/// Compare a prepared dictionary key with a retained operand without extracting its owner.
pub(crate) fn slot_key_equal(ty: &Ty, slot: &str, key: &str, columns: &str) -> String {
    if let Ty::Native(_) = ty {
        return format!("{columns}.native({slot}).equal(({key}).view())?");
    }
    if *ty == Ty::Any {
        return format!("{columns}.any(({slot}).fields()).borrowed().equal(&({key}).borrowed())?");
    }
    if let Some((fields, optional)) = matches!(ty, Ty::Tuple(_) | Ty::Struct(..))
        .then(|| hgl_source::ordinary_shape::fields(ty))
        .flatten()
    {
        let checks = fields.iter().enumerate().map(|(index, ty)| {
            let field = format!("({slot}).fields().{index}");
            let key = format!("({key}).{index}");
            if optional.contains(&index) {
                format!("match ({columns}.list(({field}).fields()).first(),({key}).as_ref()) {{(Some(layout),Some(key))=>{{let slot=hgl_store::ValueSlot::<{}>::bind(&mut layout.as_slice());{}}},(None,None)=>true,_=>false}}",crate::layouts::global_type(ty),slot_key_equal(ty,"slot","*key",columns))
            } else { slot_key_equal(ty,&field,&key,columns) }
        }).collect::<Vec<_>>();
        return if checks.is_empty() {
            "true".into()
        } else {
            checks.join(" && ")
        };
    }
    let scalar = if matches!(ty, Ty::Enum(_)) {
        "i64".into()
    } else {
        crate::layouts::owned_type(ty)
    };
    let left = format!("{columns}.scalar::<{scalar}>(({slot}).fields())");
    if *ty == Ty::Str {
        format!("({left}).as_str()==({key})")
    } else {
        format!("({left})==&({key})")
    }
}

/// Hash an exact ordinary key through its statically selected capabilities.
pub fn key_hash(ty: &Ty, value: &str) -> String {
    if let Ty::Native(scalar) = ty {
        return format!(
            "<{} as hgl_store::native_values::NativeType>::hash(({value}).view().data())?",
            crate::native_values::marker(scalar)
        );
    }
    if *ty == Ty::Any {
        return format!("({value}).key_hash()?");
    }
    if *ty == Ty::F64 {
        return format!(
            "hgl_store::map_values::hash(&if ({value})==0.0 {{0_u64}}else{{({value}).to_bits()}})"
        );
    }
    if let Some((fields, optional)) = matches!(ty, Ty::Tuple(_) | Ty::Struct(..))
        .then(|| hgl_source::ordinary_shape::fields(ty))
        .flatten()
    {
        let parts = fields
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                let field = format!("({value}).{i}");
                if optional.contains(&i) {
                    format!(
                        "u64::from(({field}).is_some()),match &{field} {{Some(value)=>{},None=>0}}",
                        key_hash(ty, "*value")
                    )
                } else {
                    format!("1,{}", key_hash(ty, &field))
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        return format!("hgl_store::map_values::combine(&[{parts}])");
    }
    format!("hgl_store::map_values::hash(&({value}))")
}
fn map_prepared(name: &str, base: &str) -> String {
    format!(
        r"
impl hgl_store::PreparedValue for {name} {{
 type Bounds=<{base} as hgl_store::PreparedValue>::Bounds;
 fn include(bounds:&mut Self::Bounds,value:&Self::Value) {{<{base} as hgl_store::PreparedValue>::include(bounds,value.entries());}}
 fn allocate(columns:&mut hgl_store::ValueColumns,bounds:&Self::Bounds)->hgl_types::NodeResult<hgl_store::ValueSlot<Self>> {{let slot=<{base} as hgl_store::PreparedValue>::allocate(columns,bounds)?;columns.prepare_map_index(slot.fields(),bounds.len)?;Ok(hgl_store::ValueSlot::from_fields(slot.fields()))}}
 fn check_native(columns:&hgl_store::ValueColumns,destination:hgl_store::ValueSlot<Self>,value:&Self::Value)->hgl_types::NodeResult {{<{base} as hgl_store::PreparedValue>::check_native(columns,hgl_store::ValueSlot::from_fields(destination.fields()),value.entries())}}
 fn check_slots(source:&hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,destination:&hgl_store::ValueColumns,to:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult {{<{base} as hgl_store::PreparedValue>::check_slots(source,hgl_store::ValueSlot::from_fields(from.fields()),destination,hgl_store::ValueSlot::from_fields(to.fields()))}}
 fn copy_native(columns:&mut hgl_store::ValueColumns,destination:hgl_store::ValueSlot<Self>,value:&Self::Value) {{<{base} as hgl_store::PreparedValue>::copy_native(columns,hgl_store::ValueSlot::from_fields(destination.fields()),value.entries());columns.copy_map_index(destination.fields(),value.index());}}
 fn copy_between(source:&hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,destination:&mut hgl_store::ValueColumns,to:hgl_store::ValueSlot<Self>) {{<{base} as hgl_store::PreparedValue>::copy_between(source,hgl_store::ValueSlot::from_fields(from.fields()),destination,hgl_store::ValueSlot::from_fields(to.fields()));destination.copy_map_index(to.fields(),source.map_index(from.fields()));}}
 fn copy_within(columns:&mut hgl_store::ValueColumns,from:hgl_store::ValueSlot<Self>,to:hgl_store::ValueSlot<Self>) {{<{base} as hgl_store::PreparedValue>::copy_within(columns,hgl_store::ValueSlot::from_fields(from.fields()),hgl_store::ValueSlot::from_fields(to.fields()));columns.copy_map_index_within(from.fields(),to.fields());}}
}}
"
    )
}
