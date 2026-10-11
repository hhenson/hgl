//! Exact ordinary collection layout and ordered constructor emission.
use hgl_semantics::ir::{Kind, Value};
use hgl_source::Ty;
use std::fmt::Write as _;
fn append(code: &mut String, args: std::fmt::Arguments<'_>) {
    code.write_fmt(args)
        .unwrap_or_else(|_| unreachable!("String formatting"));
}
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
    format!(
        r"
#[derive(Debug)] struct {name};
impl hgl_store::GlobalValue for {name} {{
 type Value=<{base} as hgl_store::GlobalValue>::Value;
 type Slots=usize;
 const WIDTH:usize=1;
 fn schema()->hgl_types::OrdinaryType {{{schema}}}
 fn slots(layout:&mut &[usize])->usize {{<{base} as hgl_store::GlobalValue>::slots(layout)}}
 fn retain(value:&Self::Value)->hgl_types::NodeResult<Self::Value> {{<{base} as hgl_store::GlobalValue>::retain(value)}}
 fn prepare(value:&Self::Value,capacity:&mut hgl_store::Capacity,layouts:&mut hgl_store::Layouts)->hgl_types::NodeResult {{<{base} as hgl_store::GlobalValue>::prepare(value,capacity,layouts)}}
 fn install(columns:&mut hgl_store::ValueColumns,value:Self::Value,layouts:&mut hgl_store::Layouts)->usize {{<{base} as hgl_store::GlobalValue>::install(columns,value,layouts)}}
 fn release(columns:&mut hgl_store::ValueColumns,slot:usize) {{<{base} as hgl_store::GlobalValue>::release(columns,slot);}}
 fn flatten(slot:usize,layout:&mut [usize]) {{<{base} as hgl_store::GlobalValue>::flatten(slot,layout);}}
 fn read(columns:&hgl_store::ValueColumns,slot:usize)->hgl_types::NodeResult<Self::Value> {{<{base} as hgl_store::GlobalValue>::read(columns,slot)}}
 fn commit(columns:&mut hgl_store::ValueColumns,slot:usize,value:Self::Value,layouts:&mut hgl_store::Layouts) {{<{base} as hgl_store::GlobalValue>::commit(columns,slot,value,layouts);}}
}}
"
    ) + &crate::prepared_values::delegate(&name, &base)
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
        let field = if child.is_some() { "&item.0" } else { "item" };
        append(
            &mut code,
            format_args!(
                "if items.iter().any(|item|{field}==&key) {{return Err(hgl_types::NodeError::new(\"duplicate ordinary collection key or member\"));}}"
            ),
        );
        if let Some(child) = child {
            append(
                &mut code,
                format_args!("let value={};items.push((key,value));", emit(child)),
            );
        } else {
            code.push_str("items.push(key);");
        }
    }
    code + "items}"
}
/// Recursive native equality with unordered complete set and map children.
pub fn equal(ty: &Ty, a: &str, b: &str, marker: fn(&Ty) -> String) -> String {
    if matches!(ty, Ty::Recursive(_)) {
        return format!("{}::ordinary_equal({a},{b})", marker(ty));
    }
    if let Ty::Family(family) = ty {
        return equal(&crate::families::family_storage(family), a, b, marker);
    }
    if let Some(item) = element(ty) {
        let compare = equal(&item, "left", "right", marker);
        return if matches!(ty, Ty::Set(_) | Ty::Map(..)) {
            format!(
                "({a}).len()==({b}).len() && ({a}).iter().all(|left|({b}).iter().any(|right|{compare}))"
            )
        } else {
            format!(
                "({a}).len()==({b}).len() && ({a}).iter().zip(({b}).iter()).all(|(left,right)|{compare})"
            )
        };
    }
    let (fields, optional) = if let Ty::Tuple(fields) = ty {
        (fields.iter().collect::<Vec<_>>(), &[][..])
    } else if let Ty::Struct(_, fields, optional) = ty {
        (
            fields.iter().map(|(_, ty)| ty).collect(),
            optional.as_slice(),
        )
    } else {
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
    if matches!(ty, Ty::Nullable(_)) {
        return format!("({source}).as_ref().map(hgl_store::Scalar::try_clone).transpose()?");
    }
    if matches!(
        ty,
        Ty::Tuple(_)
            | Ty::List(..)
            | Ty::Set(_)
            | Ty::Map(..)
            | Ty::Delta(_)
            | Ty::Recursive(_)
            | Ty::Family(_)
            | Ty::Struct(..)
    ) {
        return format!(
            "<{} as hgl_store::GlobalValue>::retain(&({source}))?",
            global_type(ty)
        );
    }
    if matches!(
        ty,
        Ty::Str | Ty::Bytes | Ty::TimeZone | Ty::Enum(_) | Ty::ZonedTime | Ty::ZonedDateTime
    ) {
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
    let (fields, optional) = if let Ty::Tuple(fields) = ty {
        (fields.iter().collect::<Vec<_>>(), &[][..])
    } else if let Ty::Struct(_, fields, optional) = ty {
        (
            fields.iter().map(|(_, ty)| ty).collect(),
            optional.as_slice(),
        )
    } else {
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
pub fn family_field(family: &hgl_source::FamilyType, index: usize, source: &str) -> String {
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
        append(
            &mut arms,
            format_args!(
                "{tag}=>&parent.{}.as_ref().expect(\"family member\").{index},",
                tag + 1
            ),
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
