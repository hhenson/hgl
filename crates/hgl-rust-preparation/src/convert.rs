//! Exact owning conversions at cold preparation and capture boundaries.
use super::ty;
use hgl_source::Ty;
fn fields(t: &Ty) -> Vec<Ty> {
    match t {
        Ty::Struct(_, fields) => fields.iter().map(|(_, t)| t.clone()).collect(),
        Ty::Tuple(ts) => ts.clone(),
        Ty::Atomic(_)
        | Ty::Map(..)
        | Ty::Delta(_)
        | Ty::List(..)
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
        | Ty::Enum(_)
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Void => unreachable!("checked aggregate"),
    }
}
/// Decode a constructed checked value into its exact native owning representation.
pub fn decode(t: &Ty, expression: &str) -> String {
    let body = match t {
        Ty::Enum(_) => "let hgl_rust_ir::Kind::Literal(hgl_source::Literal::Enum(_,number))=&v.kind else {return Err(\"prepared enum required\".into())}; *number".into(),
        Ty::Delta(origin) => return delta_decode(origin, expression),
        Ty::List(child, _) => format!(
            "let hgl_rust_ir::Kind::List(items)=&v.kind else {{return Err(\"prepared list required\".into())}}; items.iter().map(|item|Ok({})).collect::<Result<Vec<_>,String>>()?",
            decode(child, "item")
        ),
        Ty::Tuple(_) | Ty::Struct(..) => {
            let values=fields(t).iter().enumerate().map(|(i,t)|format!("{},",decode(t,&format!("items.iter().find(|(id,_)|*id=={i}).map(|(_,v)|v).ok_or(\"prepared field missing\")?")))).collect::<Vec<_>>().concat();
            format!(
                "let hgl_rust_ir::Kind::Construct(items)=&v.kind else {{return Err(\"prepared aggregate required\".into())}}; ({values})"
            )
        }
        Ty::Bool
        | Ty::I64
        | Ty::F64
        | Ty::Str
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Duration
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime => {
            let (variant, result) = match t {
                Ty::Bool => ("Bool", "*item"),
                Ty::I64 => ("Int", "*item"),
                Ty::F64 => ("Float", "*item"),
                Ty::Str => ("Str", "item.clone()"),
                Ty::Date => ("Date", "hgl_types::Date(*item)"),
                Ty::Time => ("Time", "hgl_types::Time(*item)"),
                Ty::DateTime => ("DateTime", "hgl_types::EngineTime::from_micros(*item)"),
                Ty::Duration => ("Duration", "hgl_types::EngineDelta::from_micros(*item)"),
                Ty::CivilDateTime => (
                    "CivilDateTime",
                    "hgl_types::CivilDateTime::from_micros(*item)",
                ),
                Ty::TimeZone => ("TimeZone", "item.clone()"),
                Ty::ZonedTime => ("ZonedTime", "item.clone()"),
                Ty::ZonedDateTime => ("ZonedDateTime", "item.clone()"),
                Ty::Enum(_)
                | Ty::Atomic(_)
                | Ty::Map(..)
                | Ty::Tuple(_)
                | Ty::Delta(_)
                | Ty::List(..)
                | Ty::Struct(..)
                | Ty::Ref(_)
                | Ty::Set(_)
                | Ty::Nullable(_)
                | Ty::Void => unreachable!("checked scalar"),
            };
            format!(
                "let hgl_rust_ir::Kind::Literal(hgl_source::Literal::{variant}(item))=&v.kind else {{return Err(\"prepared scalar type mismatch\".into())}}; {result}"
            )
        }
        Ty::Atomic(_) | Ty::Map(..) | Ty::Set(_) | Ty::Ref(_) | Ty::Nullable(_) | Ty::Void => {
            unreachable!("checked prepared ordinary type")
        }
    };
    format!("{{let v=&({expression}); {body}}}")
}
/// Encode an independently owned native capture as its exact checked value.
pub fn encode(t: &Ty, expression: &str) -> String {
    let kind = match t {
        Ty::Enum(_) => format!(
            "hgl_rust_ir::Kind::Literal(hgl_source::Literal::Enum({{let hgl_source::Ty::Enum(identity)={} else {{unreachable!()}};identity}},*v))",
            ty(t)
        ),
        Ty::Delta(origin) => return delta_encode(origin, expression),
        Ty::List(child, _) => format!(
            "hgl_rust_ir::Kind::List(v.iter().map(|item|{}).collect())",
            encode(child, "item")
        ),
        Ty::Struct(..) | Ty::Tuple(_) => format!(
            "hgl_rust_ir::Kind::Construct(vec![{}])",
            fields(t)
                .iter()
                .enumerate()
                .map(|(i, t)| format!("({i},{}),", encode(t, &format!("&v.{i}"))))
                .collect::<Vec<_>>()
                .concat()
        ),
        Ty::Bool
        | Ty::I64
        | Ty::F64
        | Ty::Str
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Duration
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime => {
            let (variant, expression) = match t {
                Ty::Bool => ("Bool", "*v"),
                Ty::I64 => ("Int", "*v"),
                Ty::F64 => ("Float", "*v"),
                Ty::Str => ("Str", "v.clone()"),
                Ty::Date => ("Date", "v.0"),
                Ty::Time => ("Time", "v.0"),
                Ty::DateTime => ("DateTime", "v.micros()"),
                Ty::Duration => ("Duration", "v.micros()"),
                Ty::CivilDateTime => ("CivilDateTime", "v.micros()"),
                Ty::TimeZone => ("TimeZone", "v.clone()"),
                Ty::ZonedTime => ("ZonedTime", "v.clone()"),
                Ty::ZonedDateTime => ("ZonedDateTime", "v.clone()"),
                Ty::Enum(_)
                | Ty::Atomic(_)
                | Ty::Map(..)
                | Ty::Tuple(_)
                | Ty::Delta(_)
                | Ty::List(..)
                | Ty::Struct(..)
                | Ty::Ref(_)
                | Ty::Set(_)
                | Ty::Nullable(_)
                | Ty::Void => unreachable!("checked scalar"),
            };
            format!("hgl_rust_ir::Kind::Literal(hgl_source::Literal::{variant}({expression}))")
        }
        Ty::Atomic(_) | Ty::Map(..) | Ty::Set(_) | Ty::Ref(_) | Ty::Nullable(_) | Ty::Void => {
            unreachable!("checked capture type")
        }
    };
    format!(
        "{{let v={expression}; hgl_rust_ir::Value::new({},{kind})}}",
        ty(t)
    )
}
fn member(t: &Ty, expression: &str) -> String {
    format!(
        "hgl_rust_ir::Value::new({},hgl_rust_ir::Kind::Literal(({expression}).clone()))",
        ty(t)
    )
}
fn delta_decode(origin: &Ty, expression: &str) -> String {
    let storage=match origin {
        Ty::Set(key)=>["Add","Remove"].iter().map(|tag|format!("parts.iter().filter_map(|p|if let hgl_rust_ir::DeltaEntry::{tag}(m)=p {{Some(m)}} else {{None}}).map(|m|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(key,&member(key,"m")))).collect::<Vec<_>>().concat(),
        Ty::List(child,_) | Ty::Map(_,child)=> {
            let child=child.clone().delta().unwrap_or_else(|_|unreachable!("checked child delta"));
            let mut result=vec![format!("parts.iter().filter_map(|p|if let hgl_rust_ir::DeltaEntry::Child(i,_)=p {{Some(*i)}} else {{None}}).collect::<Vec<_>>(),parts.iter().filter_map(|p|if let hgl_rust_ir::DeltaEntry::Child(_,v)=p {{Some(v)}} else {{None}}).map(|v|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(&child,"v"))];
            if matches!(origin,Ty::Map(..)) {result.push(format!("parts.iter().filter_map(|p|if let hgl_rust_ir::DeltaEntry::Remove(m)=p {{Some(m)}} else {{None}}).map(|m|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(&Ty::I64,&member(&Ty::I64,"m"))));}
            result.concat()
        }
        Ty::Tuple(_) | Ty::Struct(..)=>fields(origin).iter().enumerate().map(|(i,t)|format!("parts.iter().filter_map(|p|if let hgl_rust_ir::DeltaEntry::Child(id,v)=p {{if *id=={i} {{Some(v)}} else {{None}}}} else {{None}}).map(|v|Ok({})).collect::<Result<Vec<_>,String>>()?,",decode(&t.clone().delta().unwrap_or_else(|_|unreachable!("checked child")),"v"))).collect::<Vec<_>>().concat(),
        Ty::Atomic(_) | Ty::Delta(_) | Ty::I64 | Ty::F64 | Ty::Bool | Ty::Str | Ty::Duration | Ty::Date | Ty::Time | Ty::DateTime | Ty::CivilDateTime | Ty::TimeZone | Ty::Enum(_) | Ty::ZonedTime | Ty::ZonedDateTime | Ty::Ref(_) | Ty::Nullable(_) | Ty::Void => unreachable!("checked structural delta"),
    };
    format!(
        "{{let hgl_rust_ir::Kind::Delta(parts)=&({expression}).kind else {{return Err(\"prepared delta required\".into())}}; ({storage})}}"
    )
}
fn delta_encode(origin: &Ty, expression: &str) -> String {
    let mut body = Vec::<String>::new();
    match origin {
        Ty::Set(key) => {
            for (i, tag) in ["Add", "Remove"].iter().enumerate() {
                body.push(format!("for item in &v.{i} {{let hgl_rust_ir::Kind::Literal(member)={}.kind else {{unreachable!(\"checked member\")}}; parts.push(hgl_rust_ir::DeltaEntry::{tag}(member));}}",encode(key,"item")));
            }
        }
        Ty::List(child, _) | Ty::Map(_, child) => {
            body.push(format!("for (key,item) in v.0.iter().zip(&v.1) {{parts.push(hgl_rust_ir::DeltaEntry::Child(*key,{}));}}",encode(&child.clone().delta().unwrap_or_else(|_|unreachable!("checked child")),"item")));
            if matches!(origin, Ty::Map(..)) {
                body.push("for key in &v.2 {parts.push(hgl_rust_ir::DeltaEntry::Remove(hgl_source::Literal::Int(*key)));}".into());
            }
        }
        Ty::Tuple(_) | Ty::Struct(..) => {
            for (i, t) in fields(origin).iter().enumerate() {
                body.push(format!(
                    "for item in &v.{i} {{parts.push(hgl_rust_ir::DeltaEntry::Child({i},{}));}}",
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
        | Ty::Delta(_)
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
        | Ty::Enum(_)
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Nullable(_)
        | Ty::Void => unreachable!("checked structural delta"),
    }
    let body = body.concat();
    format!(
        "{{let v={expression}; let mut parts=Vec::new(); {body} hgl_rust_ir::Value::new({},hgl_rust_ir::Kind::Delta(parts))}}",
        ty(&Ty::Delta(Box::new(origin.clone())))
    )
}
