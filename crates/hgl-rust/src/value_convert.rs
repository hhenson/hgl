//! Exact owning conversions at cold preparation and capture boundaries.
use crate::checked_data::ty;
use hgl_source::Ty;
fn fields(t: &Ty) -> Vec<Ty> {
    hgl_semantics::any_values::fields(t)
        .unwrap_or_else(|| unreachable!("checked aggregate root"))
        .0
}
/// Decode a constructed checked value into its exact native owning representation.
pub fn decode(t: &Ty, expression: &str) -> String {
    if *t == Ty::Any {
        return format!("{{let value=&({expression});decode_any(value)?}}");
    }
    if matches!(t, Ty::Set(_) | Ty::Map(..)) {
        return decode(
            &Ty::List(
                Box::new(
                    crate::collections::element(t).unwrap_or_else(|| unreachable!("collection")),
                ),
                None,
            ),
            expression,
        );
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
        Ty::Enum(_) => "let hgl_semantics::ir::Kind::Literal(hgl_source::Literal::Enum(_,number))=&v.kind else {return Err(\"prepared enum required\".into())}; *number".into(),
        Ty::Delta(origin) => return delta_decode(origin, expression),
        Ty::List(child, _) => format!(
            "let hgl_semantics::ir::Kind::List(items)=&v.kind else {{return Err(\"prepared list required\".into())}}; items.iter().map(|item|Ok({})).collect::<Result<Vec<_>,String>>()?",
            decode(child, "item")
        ),
        Ty::Tuple(_) | Ty::Struct(..) => {
            let optional = if let Ty::Struct(_, _, optional) = t { optional.as_slice() } else { &[] };
            let values=fields(t).iter().enumerate().map(|(i,t)| {
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
    if matches!(t, Ty::Set(_) | Ty::Map(..)) {
        return format!(
            "{{let mut value={};value.ty={};value}}",
            encode(
                &Ty::List(
                    Box::new(
                        crate::collections::element(t)
                            .unwrap_or_else(|| unreachable!("collection"))
                    ),
                    None
                ),
                expression
            ),
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
    let kind = if matches!(t, Ty::Enum(_)) {
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
        Ty::Tuple(_) | Ty::Struct(..)=>fields(origin).iter().enumerate().map(|(i,t)|format!("parts.iter().filter_map(|p|if let hgl_semantics::ir::DeltaEntry::Child(id,v)=p {{if *id=={i} {{Some(v)}} else {{None}}}} else {{None}}).map(|v|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(&t.clone().delta().unwrap_or_else(|_|unreachable!("checked child")),"v"))).collect::<Vec<_>>().concat(),
        Ty::Atomic(_) | Ty::Rolling(..) | Ty::Delta(_) | Ty::I64 | Ty::F64 | Ty::Bool | Ty::Str | Ty::Bytes | Ty::Any | Ty::Duration | Ty::Date | Ty::Time | Ty::DateTime | Ty::CivilDateTime | Ty::TimeZone | Ty::Enum(_) | Ty::ZonedTime | Ty::ZonedDateTime | Ty::Ref(_) | Ty::Nullable(_) | Ty::Recursive(_) | Ty::Family(_) | Ty::Void => unreachable!("checked structural delta"),
    };
    format!(
        "{{let hgl_semantics::ir::Kind::Delta(parts)=&({expression}).kind else {{return Err(\"prepared delta required\".into())}}; ({storage})}}"
    )
}
fn delta_encode(origin: &Ty, expression: &str) -> String {
    let mut body = Vec::<String>::new();
    match origin {
        Ty::Set(key) => {
            for (i, tag) in ["Add", "Remove"].iter().enumerate() {
                body.push(format!(
                    "for item in &v.{i} {{parts.push(hgl_semantics::ir::DeltaEntry::{tag}({}));}}",
                    encode(key, "item")
                ));
            }
        }
        Ty::List(child, size) => {
            if size.is_none() {
                body.push(format!(
                    "for key in &v.2 {{parts.push(hgl_semantics::ir::DeltaEntry::Remove({}));}}",
                    encode(&Ty::I64, "key")
                ));
            }
            body.push(format!("for (key,item) in v.0.iter().zip(&v.1) {{parts.push(hgl_semantics::ir::DeltaEntry::Child(*key,{}));}}",encode(&child.clone().delta().unwrap_or_else(|_|unreachable!("checked child")),"item")));
        }
        Ty::Map(key, child) => {
            body.push(format!("for (key,item) in v.0.iter().zip(&v.1) {{parts.push(hgl_semantics::ir::DeltaEntry::Keyed({},{}));}}",encode(key,"key"),encode(&child.clone().delta().unwrap_or_else(|_|unreachable!("checked child")),"item")));
            body.push(format!(
                "for key in &v.2 {{parts.push(hgl_semantics::ir::DeltaEntry::Remove({}));}}",
                encode(key, "key")
            ));
        }
        Ty::Tuple(_) | Ty::Struct(..) => {
            for (i, t) in fields(origin).iter().enumerate() {
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
        }
        Ty::Atomic(_)
        | Ty::Rolling(..)
        | Ty::Delta(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Bytes
        | Ty::Any
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::Enum(_)
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Nullable(_)
        | Ty::Recursive(_)
        | Ty::Family(_)
        | Ty::Void => unreachable!("checked structural delta"),
    }
    let body = body.concat();
    format!(
        "{{let v={expression}; let mut parts=Vec::new(); {body} hgl_semantics::ir::Value::new({},hgl_semantics::ir::Kind::Delta(parts))}}",
        ty(&Ty::Delta(Box::new(origin.clone())))
    )
}

fn encode_fields(t: &Ty) -> String {
    let optional = if let Ty::Struct(_, _, optional) = t {
        optional.as_slice()
    } else {
        &[]
    };
    let entries = fields(t)
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
fn scalar_codec(t: &Ty) -> (&'static str, &'static str, &'static str) {
    match t
        .scalar_kind()
        .unwrap_or_else(|| unreachable!("checked scalar"))
    {
        hgl_types::ScalarType::Bool => ("Bool", "*item", "*v"),
        hgl_types::ScalarType::I64 => ("Int", "*item", "*v"),
        hgl_types::ScalarType::F64 => ("Float", "*item", "*v"),
        hgl_types::ScalarType::Text => ("Str", "item.clone()", "v.clone()"),
        hgl_types::ScalarType::Bytes => ("Bytes", "item.clone()", "v.clone()"),
        hgl_types::ScalarType::Date => ("Date", "hgl_types::Date(*item)", "v.0"),
        hgl_types::ScalarType::Time => ("Time", "hgl_types::Time(*item)", "v.0"),
        hgl_types::ScalarType::DateTime => (
            "DateTime",
            "hgl_types::EngineTime::from_micros(*item)",
            "v.micros()",
        ),
        hgl_types::ScalarType::Duration => (
            "Duration",
            "hgl_types::EngineDelta::from_micros(*item)",
            "v.micros()",
        ),
        hgl_types::ScalarType::CivilDateTime => (
            "CivilDateTime",
            "hgl_types::CivilDateTime::from_micros(*item)",
            "v.micros()",
        ),
        hgl_types::ScalarType::TimeZone => ("TimeZone", "item.clone()", "v.clone()"),
        hgl_types::ScalarType::ZonedTime => ("ZonedTime", "item.clone()", "v.clone()"),
        hgl_types::ScalarType::ZonedDateTime => ("ZonedDateTime", "item.clone()", "v.clone()"),
    }
}
