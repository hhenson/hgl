//! Typed owned current tuple observations, preserving every child's validity.
use crate::layouts::global_type;
use hgl_semantics::ir::Value;
use hgl_source::Ty;
use std::fmt::Write as _;
fn optional(ty: &Ty) -> Ty {
    Ty::Struct(
        format!("\0snapshot-child<{}>", ty.source_name()).into(),
        vec![("value".into(), storage(ty))],
        vec![0],
    )
}
/// Private emission-only storage; canonical HGL tuple identity never changes.
pub fn storage(ty: &Ty) -> Ty {
    match ty {
        Ty::Tuple(children) => Ty::Struct(
            format!("\0snapshot<{}>", ty.source_name()).into(),
            children
                .iter()
                .enumerate()
                .map(|(i, child)| (i.to_string(), storage(child)))
                .collect(),
            (0..children.len()).collect(),
        ),
        Ty::List(child, size) => Ty::List(Box::new(optional(child)), *size),
        Ty::Map(key, child) => Ty::Map(key.clone(), Box::new(optional(child))),
        Ty::Rolling(..)
        | Ty::Family(_)
        | Ty::Recursive(_)
        | Ty::Enum(_)
        | Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::Struct(..)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Void => ty.clone(),
    }
}
fn reserve(collection: &str, count: &str) -> String {
    format!(
        "{collection}.try_reserve({count}).map_err(|e|hgl_types::NodeError::new(e.to_string()))?;"
    )
}
/// Turn a complete ordinary native value into the same typed snapshot representation.
pub fn complete(ty: &Ty, value: &str) -> String {
    match ty {
        Ty::Tuple(children) => format!(
            "{{let value={value};({},)}}",
            children
                .iter()
                .enumerate()
                .map(|(i, ty)| format!("Some({})", complete(ty, &format!("value.{i}"))))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Ty::List(child, _) => format!(
            "{{let source={value};let mut values=Vec::new();{}for value in source {{values.push((Some({}),));}}values}}",
            reserve("values", "source.len()"),
            complete(child, "value")
        ),
        Ty::Map(_, child) => format!(
            "{{let source={value};let mut values=Vec::new();{}for (key,value) in source {{values.push((key,(Some({}),)));}}values}}",
            reserve("values", "source.len()"),
            complete(child, "value")
        ),
        Ty::Rolling(..)
        | Ty::Family(_)
        | Ty::Recursive(_)
        | Ty::Enum(_)
        | Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::Struct(..)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Void => value.into(),
    }
}
/// Publish the complete held value, using existing child invalidation semantics.
pub fn publish(ty: &Ty, output: &str, value: &str) -> String {
    match ty {
        Ty::Tuple(children) => {
            let mut code = format!("{{let output={output};let value={value};");
            for (i, ty) in children.iter().enumerate() {
                write!(&mut code,
                    "{{let output=output.field::<{i}>(_ctx.store().bindings());if let Some(value)=value.{i} {{{}}}}}",
                    publish(ty, "output", "value")
                ).unwrap_or_else(|_|unreachable!("String formatting"));
            }
            code + "}"
        }
        Ty::List(child, Some(_)) => format!(
            "{{let output={output};for (index,(value,)) in ({value}).into_iter().enumerate() {{let output=output.index(_ctx.store().bindings(),index);if let Some(value)=value {{{}}}}}}}",
            publish(child, "output", "value")
        ),
        Ty::Map(key, child) => keyed_publish(key, child, output, value),
        Ty::List(child, None) => keyed_publish(&Ty::I64, child, output, value),
        Ty::Rolling(..)
        | Ty::Family(_)
        | Ty::Recursive(_)
        | Ty::Enum(_)
        | Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::Struct(..)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Void => {
            if let Some(ty) = crate::layouts::whole_payload(ty) {
                format!("_ctx.set_atomic::<{}>({output},{value})?;", global_type(ty))
            } else {
                format!("_ctx.set(hgl_store::Store::prepared_output({output}),{value});")
            }
        }
    }
}
fn keyed_publish(key: &Ty, child: &Ty, output: &str, value: &str) -> String {
    format!(
        "{{let output={output};let values={value};for(key,(value,)) in values {{let key=<{} as hgl_store::Key>::id(&_ctx.store().keys,&key)?;_ctx.get_or_create_shaped(output.id(),key);let output=output.member(_ctx.store().bindings(),key).ok_or_else(||hgl_types::NodeError::new(\"missing created member\"))?;if let Some(value)=value {{{}}}}}}}",
        global_type(key),
        publish(child, "output", "value")
    )
}
fn original(ty: &Ty) -> Option<Ty> {
    if let Ty::Struct(name, fields, _) = ty
        && name.source_name().starts_with("\0snapshot<")
    {
        Some(Ty::Tuple(
            fields
                .iter()
                .map(|(_, child)| original(child).unwrap_or_else(|| child.clone()))
                .collect(),
        ))
    } else if let Ty::List(wrapper, size) = ty {
        unwrap(wrapper).map(|child| Ty::List(Box::new(child), *size))
    } else if let Ty::Map(key, wrapper) = ty {
        unwrap(wrapper).map(|child| Ty::Map(key.clone(), Box::new(child)))
    } else {
        None
    }
}
fn unwrap(ty: &Ty) -> Option<Ty> {
    if let Ty::Struct(name, fields, _) = ty
        && name.source_name().starts_with("\0snapshot-child<")
    {
        Some(original(&fields[0].1).unwrap_or_else(|| fields[0].1.clone()))
    } else {
        None
    }
}
/// Exact finite snapshot bounds derived from existing typed limits and child domains.
pub fn bounds(
    ty: &Ty,
    scalar: impl Fn(&Ty) -> String,
    domain: impl Fn(&Ty) -> String,
) -> Option<String> {
    fn build(ty: &Ty, scalar: &impl Fn(&Ty) -> String, domain: &impl Fn(&Ty) -> String) -> String {
        let marker = global_type(&storage(ty));
        match ty {
            Ty::Tuple(children) => format!(
                "PreparedBounds{marker} {{{}}}",
                children
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| format!("field{i}:Some({})", build(ty, scalar, domain)))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Ty::List(child, size) => format!(
                "hgl_store::ListBounds {{len:{},element:PreparedBounds{} {{field0:Some({})}}}}",
                size.map_or_else(|| domain(ty), |n| n.to_string()),
                global_type(&if let Ty::List(wrapper, _) = storage(ty) {
                    *wrapper
                } else {
                    unreachable!()
                }),
                build(child, scalar, domain)
            ),
            Ty::Map(key, child) => {
                let Ty::Map(_, wrapper) = storage(ty) else {
                    unreachable!()
                };
                let pair = Ty::Tuple(vec![(**key).clone(), (*wrapper).clone()]);
                format!(
                    "hgl_store::ListBounds {{len:{},element:PreparedBounds{} {{field0:{},field1:PreparedBounds{} {{field0:Some({})}}}}}}",
                    domain(ty),
                    global_type(&pair),
                    scalar(key),
                    global_type(&wrapper),
                    build(child, scalar, domain)
                )
            }
            Ty::Rolling(..)
            | Ty::Family(_)
            | Ty::Recursive(_)
            | Ty::Enum(_)
            | Ty::Atomic(_)
            | Ty::Delta(_)
            | Ty::Struct(..)
            | Ty::I64
            | Ty::F64
            | Ty::Bool
            | Ty::Str
            | Ty::Duration
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::CivilDateTime
            | Ty::TimeZone
            | Ty::ZonedTime
            | Ty::ZonedDateTime
            | Ty::Ref(_)
            | Ty::Set(_)
            | Ty::Nullable(_)
            | Ty::Void => scalar(ty),
        }
    }
    original(ty).map(|ty| build(&ty, &scalar, &domain))
}
/// Retain complete native aggregate children once before positional assembly.
pub fn ordinary(ty: &Ty, fields: &[(usize, Value)], emit: impl Fn(&Value) -> String) -> String {
    let (count, optional) = if let Ok((_, all, optional)) = ty.structure() {
        (all.len(), optional)
    } else if let Ty::Tuple(all) = ty {
        (all.len(), &[][..])
    } else {
        unreachable!("aggregate constructor")
    };
    if count == 0 {
        return "()".into();
    }
    let mut code = vec!["{ ".to_owned()];
    for (index, argument) in fields {
        code.push(format!("let field{index} = {}; ", emit(argument)));
    }
    let fields = (0..count)
        .map(|index| {
            if optional.contains(&index) {
                if fields.iter().any(|(i, _)| *i == index) {
                    if ty.structure().is_ok_and(|(_,all,_)|matches!(&all[index].1,Ty::Recursive(batch) if batch.definitions().is_empty())) {
                        format!("Some(Box::new(field{index}))")
                    } else { format!("Some(field{index})") }
                } else {
                    "None".into()
                }
            } else {
                format!("field{index}")
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let prefix = if matches!(ty, Ty::Recursive(_) | Ty::Family(_)) {
        format!("Owned{}", global_type(ty))
    } else {
        String::new()
    };
    code.push(format!("{prefix}({fields},) }}"));
    code.concat()
}
