//! Typed owned current tuple observations, preserving every child's validity.
use crate::layouts::global_type;
use hgl_semantics::ir::Value;
use hgl_source::Ty;
/// Declared positional children; this cold representation preserves canonical source fields.
pub fn children(ty: &Ty) -> Option<Vec<&Ty>> {
    if let Ty::Tuple(children) = ty {
        Some(children.iter().collect())
    } else if let Ty::Struct(_, fields, _) = ty {
        Some(fields.iter().map(|(_, child)| child).collect())
    } else {
        None
    }
}
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
        Ty::Struct(_, fields, _) => Ty::Struct(
            hgl_source::type_shape::Nominal {
                origin: "\0snapshot-struct".into(),
                arguments: vec![ty.clone()],
            },
            fields
                .iter()
                .map(|(name, child)| (name.clone(), storage(child)))
                .collect(),
            (0..fields.len()).collect(),
        ),
        Ty::List(child, size) => Ty::List(Box::new(optional(child)), *size),
        Ty::Map(key, child) => Ty::Map(key.clone(), Box::new(optional(child))),
        Ty::Rolling(..)
        | Ty::Family(_)
        | Ty::Recursive(_)
        | Ty::Enum(_)
        | Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Bytes
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
    if let Some(children) = children(ty) {
        let optional = ty.structure().map_or(&[][..], |(_, _, optional)| optional);
        let fields = children
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                if optional.contains(&i) {
                    format!("value.{i}.map(|value|{})", complete(ty, "value"))
                } else {
                    format!("Some({})", complete(ty, &format!("value.{i}")))
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        return format!("{{let value={value};({fields},)}}");
    }
    match ty {
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
        Ty::Tuple(_) | Ty::Struct(..) => unreachable!("positional children handled above"),
        Ty::Rolling(..)
        | Ty::Family(_)
        | Ty::Recursive(_)
        | Ty::Enum(_)
        | Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Bytes
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
/// Publish a complete native value through the structural publication owner.
pub fn publish(ty: &Ty, output: &str, value: &str) -> String {
    crate::structural_publication::native(ty, output, value)
}
fn original(ty: &Ty) -> Option<Ty> {
    if let Ty::Struct(name, _, _) = ty
        && name.origin == "\0snapshot-struct"
    {
        return name.arguments.first().cloned();
    }
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
        if let Some(children) = children(ty) {
            return format!(
                "PreparedBounds{marker} {{{}}}",
                children
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| format!("field{i}:Some({})", build(ty, scalar, domain)))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        match ty {
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
            Ty::Tuple(_) | Ty::Struct(..) => unreachable!("positional children handled above"),
            Ty::Rolling(..)
            | Ty::Family(_)
            | Ty::Recursive(_)
            | Ty::Enum(_)
            | Ty::Atomic(_)
            | Ty::Delta(_)
            | Ty::I64
            | Ty::F64
            | Ty::Bool
            | Ty::Str
            | Ty::Bytes
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
