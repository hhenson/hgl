//! Static source-to-native scalar spellings.
use hgl_source::{Literal, Ty};
/// Emit the checked rust type form.
pub fn rust_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::Enum(_) | Ty::I64 => "i64",
        Ty::Duration => "hgl_types::EngineDelta",
        Ty::Date => "hgl_types::Date",
        Ty::Time => "hgl_types::Time",
        Ty::DateTime => "hgl_types::EngineTime",
        Ty::CivilDateTime => "hgl_types::CivilDateTime",
        Ty::TimeZone => "hgl_types::ZoneId",
        Ty::ZonedTime => "hgl_types::ZonedTime",
        Ty::ZonedDateTime => "hgl_types::ZonedDateTime",
        Ty::Bool => "bool",
        Ty::F64 => "f64",
        Ty::Str => "String",
        Ty::Bytes => "Vec<u8>",
        Ty::Void => "()",
        Ty::Atomic(_)
        | Ty::Rolling(..)
        | Ty::Map(..)
        | Ty::Tuple(_)
        | Ty::Delta(_)
        | Ty::Recursive(_)
        | Ty::Family(_)
        | Ty::Struct(..)
        | Ty::List(..)
        | Ty::Nullable(_) => {
            unreachable!("ordinary aggregate and nullable locals use inferred Rust types")
        }
        Ty::Ref(_) => "hgl_store::Reference",
        Ty::Set(_) => "hgl_store::InputId",
    }
}
/// Emit the checked scalar type form.
pub fn scalar_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::Enum(_) => unreachable!("enum metadata retains nominal identity"),
        Ty::Bool => "Bool",
        Ty::F64 => "F64",
        Ty::I64 => "I64",
        Ty::Str => "Text",
        Ty::Bytes => "Bytes",
        Ty::Duration => "Duration",
        Ty::Date => "Date",
        Ty::Time => "Time",
        Ty::DateTime => "DateTime",
        Ty::CivilDateTime => "CivilDateTime",
        Ty::TimeZone => "TimeZone",
        Ty::ZonedTime => "ZonedTime",
        Ty::ZonedDateTime => "ZonedDateTime",
        Ty::Atomic(_)
        | Ty::Rolling(..)
        | Ty::Map(..)
        | Ty::Tuple(_)
        | Ty::Delta(_)
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Recursive(_)
        | Ty::Family(_)
        | Ty::Struct(..)
        | Ty::List(..)
        | Ty::Void => {
            unreachable!("checked endpoint type")
        }
    }
}

/// Emit the checked literal form.
pub fn literal(value: &Literal) -> String {
    match value {
        Literal::Int(i) => format!("{i}_i64"),
        Literal::Duration(i) => format!("hgl_types::EngineDelta::from_micros({i})"),
        Literal::Date(i) => format!("hgl_types::Date({i})"),
        Literal::Time(i) => format!("hgl_types::Time({i})"),
        Literal::DateTime(i) => format!("hgl_types::EngineTime::from_micros({i})"),
        Literal::CivilDateTime(i) => format!("hgl_types::CivilDateTime::from_micros({i})"),
        Literal::TimeZone(zone) => format!(
            "hgl_types::ZoneId::from_validated_name({:?}.to_owned())",
            zone.as_str()
        ),
        Literal::Enum(_, number) => format!("{number}_i64"),
        Literal::ZonedTime(value) => format!(
            "hgl_types::ZonedTime::from_validated_parts(hgl_types::Time({}), hgl_types::ZoneId::from_validated_name({:?}.to_owned()))",
            value.time().0,
            value.zone().as_str()
        ),
        Literal::ZonedDateTime(value) => format!(
            "hgl_types::ZonedDateTime::from_validated_parts(hgl_types::EngineTime::from_micros({}), hgl_types::ZoneId::from_validated_name({:?}.to_owned()), {})",
            value.instant().micros(),
            value.zone().as_str(),
            value.offset_seconds()
        ),
        Literal::Bool(b) => b.to_string(),
        Literal::Float(f) if !f.is_finite() => format!("f64::from_bits({})", f.to_bits()),
        Literal::Float(f) => format!("{f:?}_f64"),
        Literal::Str(s) => format!("{s:?}.to_owned()"),
        Literal::Bytes(bytes) => format!("vec!{bytes:?}"),
    }
}

/// Select a direct ordinary list source through a pure bytes helper's one return.
pub fn bytes_input(value: &hgl_semantics::ir::Value) -> Option<usize> {
    use hgl_semantics::ir::{Kind, Statement};
    let argument = if let Kind::Unary(op, argument) = &value.kind
        && op == "bytes"
    {
        argument.as_ref()
    } else if let Kind::ValueCall(args, body) = &value.kind {
        let [supplied] = args.as_slice() else {
            return None;
        };
        let [Statement::Yield(result)] = body.as_slice() else {
            return None;
        };
        let Kind::Unary(op, argument) = &result.kind else {
            return None;
        };
        let Kind::Local(id) = argument.kind else {
            return None;
        };
        if op != "bytes" || id != 0 {
            return None;
        }
        supplied
    } else {
        return None;
    };
    let argument = if let Kind::Unary(op, argument) = &argument.kind
        && op == "atomic_value"
    {
        argument.as_ref()
    } else {
        argument
    };
    if let Kind::Input(id, _) = argument.kind
        && let Ty::Atomic(payload) = &argument.ty
        && matches!(payload.as_ref(),Ty::List(child,_) if **child==Ty::I64)
    {
        Some(id)
    } else {
        None
    }
}

/// Emit the established checked integer arithmetic policy.
pub fn integer_binary(op: &str, a: &str, b: &str) -> String {
    match op {
        "+" => format!("(({a}).wrapping_add({b}))"),
        "-" => format!("(({a}).wrapping_sub({b}))"),
        "*" => format!("(({a}).wrapping_mul({b}))"),
        "%" => format!(
            "{{ let lhs = {a}; let rhs = {b}; if rhs == -1 {{ 0_i64 }} else {{ let rem = lhs.checked_rem(rhs).ok_or_else(|| hgl_kernel::NodeError::new(\"modulo by zero\"))?; if rem != 0 && (rem < 0) != (rhs < 0) {{ rem + rhs }} else {{ rem }} }} }}"
        ),
        _ => unreachable!("checked integer operation"),
    }
}
