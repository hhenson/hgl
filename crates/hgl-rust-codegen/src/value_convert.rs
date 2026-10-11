//! Exact owning conversions at cold preparation and capture boundaries.
use crate::checked_data::ty;
use hgl_source::Ty;
fn fields(t: &Ty) -> (Vec<Ty>, Vec<usize>) {
    hgl_semantics::any_values::fields(t).unwrap_or_else(|| unreachable!("checked aggregate root"))
}
/// Decode a constructed checked value into its exact native owning representation.
pub fn decode(t: &Ty, expression: &str) -> String {
    if *t == Ty::Any {
        return format!("{{let value=&({expression});decode_any(value)?}}");
    }
    if let Some(storage) = collection_storage(t) {
        return decode(&storage, expression);
    }
    if let Ty::Family(family) = t {
        return crate::families::decode(family, expression, decode);
    }
    if matches!(t, Ty::Recursive(_)) {
        return format!(
            "{}::decode_recursive({expression})?",
            crate::layouts::global_type(t)
        );
    }
    let body = match t {
        Ty::Native(scalar) => crate::native_values::decode(scalar),
        Ty::Enum(_) => "let hgl_semantics::ir::Kind::Literal(hgl_source::Literal::Enum(_,number))=&v.kind else {return Err(\"prepared enum required\".into())}; *number".into(),
        Ty::Delta(origin) => return delta_decode(origin, expression),
        Ty::List(child, _) => format!(
            "let hgl_semantics::ir::Kind::List(items)=&v.kind else {{return Err(\"prepared list required\".into())}}; items.iter().map(|item|Ok({})).collect::<Result<Vec<_>,String>>()?",
            decode(child, "item")
        ),
        Ty::Tuple(_) | Ty::Struct(..) => {
            let (fields, optional) = fields(t);
            let values=fields.iter().enumerate().map(|(i,t)| {
                let found=format!("items.iter().find(|(id,_)|*id=={i}).map(|(_,v)|v)");
                if optional.contains(&i) { format!("{found}.map(|field|Ok::<_,String>({})).transpose()?,",decode(t,"field")) }
                else {format!("{},",decode(t,&format!("{found}.ok_or(\"prepared field missing\")?")))}
            }).collect::<Vec<_>>().concat();
            format!(
                "let hgl_semantics::ir::Kind::Construct(items)=&v.kind else {{return Err(\"prepared aggregate required\".into())}}; ({values})"
            )
        }
        Ty::Rolling(..) | Ty::Family(_) | Ty::Recursive(_) | Ty::Atomic(_) | Ty::Map(..) | Ty::I64 | Ty::F64 | Ty::Bool | Ty::Str | Ty::Bytes | Ty::Any | Ty::Duration | Ty::Date | Ty::Time | Ty::DateTime | Ty::CivilDateTime | Ty::TimeZone | Ty::ZonedTime | Ty::ZonedDateTime | Ty::Ref(_) | Ty::Set(_) | Ty::Nullable(_) | Ty::Void => scalar_decode(t),
    };
    format!("{{let v=&({expression}); {body}}}")
}
/// Encode an independently owned native capture as its exact checked value.
pub fn encode(t: &Ty, expression: &str) -> String {
    if *t == Ty::Any {
        return format!("encode_any({expression})");
    }
    if let Some(storage) = collection_storage(t) {
        return format!(
            "{{let mut value={};value.ty={};value}}",
            encode(&storage, expression),
            ty(t)
        );
    }
    if let Ty::Family(family) = t {
        return crate::families::encode(family, expression, encode);
    }
    if matches!(t, Ty::Recursive(_)) {
        return format!(
            "{}::encode_recursive({expression})",
            crate::layouts::global_type(t)
        );
    }
    let kind = if let Ty::Native(scalar) = t {
        crate::native_values::encode(scalar)
    } else if matches!(t, Ty::Enum(_)) {
        format!(
            "hgl_semantics::ir::Kind::Literal(hgl_source::Literal::Enum({{let hgl_source::Ty::Enum(identity)={} else {{unreachable!()}};identity}},*v))",
            ty(t)
        )
    } else if let Ty::Delta(origin) = t {
        return delta_encode(origin, expression);
    } else if let Ty::List(child, _) = t {
        format!(
            "hgl_semantics::ir::Kind::List(v.iter().map(|item|{}).collect())",
            encode(child, "item")
        )
    } else if matches!(t, Ty::Struct(..) | Ty::Tuple(_)) {
        encode_fields(t)
    } else {
        scalar_encode(t)
    };
    format!(
        "{{let v={expression}; hgl_semantics::ir::Value::new({},{kind})}}",
        ty(t)
    )
}
fn delta_decode(origin: &Ty, expression: &str) -> String {
    let storage=match origin {
        Ty::Set(key)=>["Add","Remove"].iter().map(|tag|format!("parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::{tag}(m)=p {{Some(m)}} else {{None}}).map(|m|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(key,"m"))).collect::<Vec<_>>().concat(),
        Ty::List(child,size) => {
            let removed=if size.is_none() {format!("parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::Remove(k)=p {{Some(k)}} else {{None}}).map(|k|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(&Ty::I64,"k"))}else{String::new()};
            let child=child.clone().delta().unwrap_or_else(|_|unreachable!("checked child delta"));
            format!("parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::Child(i,_)=p {{Some(*i)}} else {{None}}).collect::<Vec<_>>(),parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::Child(_,v)=p {{Some(v)}} else {{None}}).map(|v|Ok({})).collect::<Result<Vec<_>,String>>()?,{removed}",decode(&child,"v"))
        }
        Ty::Map(key,child)=> {
            let child=child.clone().delta().unwrap_or_else(|_|unreachable!("checked child delta"));
            format!("parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::Keyed(k,_)=p {{Some(k)}} else {{None}}).map(|k|Ok({})).collect::<Result<Vec<_>,String>>()?,parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::Keyed(_,v)=p {{Some(v)}} else {{None}}).map(|v|Ok({})).collect::<Result<Vec<_>,String>>()?,parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::Remove(k)=p {{Some(k)}} else {{None}}).map(|k|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(key,"k"),decode(&child,"v"),decode(key,"k"))
        }
        Ty::Tuple(_) | Ty::Struct(..)=>fields(origin).0.iter().enumerate().map(|(i,t)|format!("parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::Child(id,v)=p {{if *id=={i} {{Some(v)}} else {{None}}}} else {{None}}).map(|v|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(&t.clone().delta().unwrap_or_else(|_|unreachable!("checked child")),"v"))).collect::<Vec<_>>().concat(),
        Ty::Atomic(_) | Ty::Rolling(..) | Ty::Delta(_) | Ty::I64 | Ty::F64 | Ty::Bool | Ty::Str | Ty::Bytes | Ty::Any | Ty::Duration | Ty::Date | Ty::Time | Ty::DateTime | Ty::CivilDateTime | Ty::TimeZone | Ty::Enum(_) | Ty::Native(_) | Ty::ZonedTime | Ty::ZonedDateTime | Ty::Ref(_) | Ty::Nullable(_) | Ty::Recursive(_) | Ty::Family(_) | Ty::Void => unreachable!("checked structural delta"),
    };
    format!(
        "{{let hgl_semantics::ir::Kind::Delta(parts)=&({expression}).kind else {{return Err(\"prepared delta required\".into())}}; ({storage})}}"
    )
}
fn delta_encode(origin: &Ty, expression: &str) -> String {
    let mut body = Vec::<String>::new();
    if let Ty::Set(key) = origin {
        for (i, tag) in ["Add", "Remove"].iter().enumerate() {
            body.push(format!(
                "for item in &v.{i} {{parts.push(hgl_semantics::ir::DeltaEntry::{tag}({}));}}",
                encode(key, "item")
            ));
        }
    } else if let Ty::List(child, size) = origin {
        if size.is_none() {
            body.push(format!(
                "for key in &v.2 {{parts.push(hgl_semantics::ir::DeltaEntry::Remove({}));}}",
                encode(&Ty::I64, "key")
            ));
        }
        body.push(format!("for (key,item) in v.0.iter().zip(&v.1) {{parts.push(hgl_semantics::ir::DeltaEntry::Child(*key,{}));}}",encode(&child.clone().delta().unwrap_or_else(|_|unreachable!("checked child")),"item")));
    } else if let Ty::Map(key, child) = origin {
        body.push(format!("for (key,item) in v.0.iter().zip(&v.1) {{parts.push(hgl_semantics::ir::DeltaEntry::Keyed({},{}));}}",encode(key,"key"),encode(&child.clone().delta().unwrap_or_else(|_|unreachable!("checked child")),"item")));
        body.push(format!(
            "for key in &v.2 {{parts.push(hgl_semantics::ir::DeltaEntry::Remove({}));}}",
            encode(key, "key")
        ));
    } else if matches!(origin, Ty::Tuple(_) | Ty::Struct(..)) {
        for (i, t) in fields(origin).0.iter().enumerate() {
            body.push(format!(
                "for item in &v.{i} {{parts.push(hgl_semantics::ir::DeltaEntry::Child({i},{}));}}",
                encode(
                    &t.clone()
                        .delta()
                        .unwrap_or_else(|_| unreachable!("checked child")),
                    "item"
                )
            ));
        }
    } else {
        unreachable!("checked structural delta");
    }
    let body = body.concat();
    format!(
        "{{let v={expression}; let mut parts=Vec::new(); {body} hgl_semantics::ir::Value::new({},hgl_semantics::ir::Kind::Delta(parts))}}",
        ty(&Ty::Delta(Box::new(origin.clone())))
    )
}

fn encode_fields(t: &Ty) -> String {
    let (fields, optional) = fields(t);
    let entries = fields
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if optional.contains(&i) {
                format!(
                    "if let Some(field)=&v.{i} {{ fields.push(({i},{})); }}",
                    encode(t, "field")
                )
            } else {
                format!("fields.push(({i},{}));", encode(t, &format!("&v.{i}")))
            }
        })
        .collect::<Vec<_>>()
        .concat();
    format!("{{let mut fields=Vec::new(); {entries} hgl_semantics::ir::Kind::Construct(fields)}}")
}

fn scalar_decode(t: &Ty) -> String {
    let (variant, decode, _) = scalar_codec(t);
    format!(
        "let hgl_semantics::ir::Kind::Literal(hgl_source::Literal::{variant}(item))=&v.kind else {{return Err(\"prepared scalar type mismatch\".into())}}; {decode}"
    )
}
fn scalar_encode(t: &Ty) -> String {
    let (variant, _, encode) = scalar_codec(t);
    format!("hgl_semantics::ir::Kind::Literal(hgl_source::Literal::{variant}({encode}))")
}
fn scalar_codec(t: &Ty) -> (String, String, &'static str) {
    use hgl_types::ScalarType;
    let kind = t
        .scalar_kind()
        .unwrap_or_else(|| unreachable!("checked scalar"));
    let (variant, decoded, encoded) = match kind {
        ScalarType::Bool => ("Bool", "*item", "*v"),
        ScalarType::I64 => ("Int", "*item", "*v"),
        ScalarType::F64 => ("Float", "*item", "*v"),
        ScalarType::Text => ("Str", "item.clone()", "v.clone()"),
        ScalarType::Date | ScalarType::Time => {
            return (
                format!("{kind:?}"),
                format!("{}(*item)", crate::scalars::rust_type(t)),
                "v.0",
            );
        }
        ScalarType::DateTime | ScalarType::Duration | ScalarType::CivilDateTime => {
            return (
                format!("{kind:?}"),
                format!("{}::from_micros(*item)", crate::scalars::rust_type(t)),
                "v.micros()",
            );
        }
        ScalarType::Bytes
        | ScalarType::TimeZone
        | ScalarType::ZonedTime
        | ScalarType::ZonedDateTime => {
            return (format!("{kind:?}"), "item.clone()".into(), "v.clone()");
        }
    };
    (variant.into(), decoded.into(), encoded)
}

fn collection_storage(ty: &Ty) -> Option<Ty> {
    if !matches!(ty, Ty::Set(_) | Ty::Map(..)) {
        return None;
    }
    Some(Ty::List(
        Box::new(
            crate::collections::element(ty).unwrap_or_else(|| unreachable!("checked collection")),
        ),
        None,
    ))
}
