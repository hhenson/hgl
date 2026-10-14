//! Checked ordinary operator type algebra, independent of execution and source traversal.
use hgl_source::Ty;
/// Resolve the admitted binary table after any numeric operand widening.
pub fn binary_type(op: &str, a: &Ty, b: &Ty) -> Result<Ty, String> {
    match (op, a, b) {
        ("+" | "-", Ty::DateTime, Ty::Duration) | ("+", Ty::Duration, Ty::DateTime) => {
            return Ok(Ty::DateTime);
        }
        ("-", Ty::DateTime, Ty::DateTime) | ("+" | "-", Ty::Duration, Ty::Duration) => {
            return Ok(Ty::Duration);
        }
        _ => {}
    }
    if *a != *b {
        return Err("binary operand type mismatch".into());
    }
    if matches!(a, Ty::Result(..))
        && matches!(op, "==" | "!=")
        && !hgl_source::ordinary_shape::capable(a, 0)
    {
        return Err("result equality requires equality of both payload types".into());
    }
    if let Ty::Native(native) = a {
        let capability = match op {
            "==" | "!=" => "equality",
            "<" | "<=" | ">" | ">=" => "order",
            _ => return Err(format!("unsupported native scalar operation {op}")),
        };
        return native
            .capabilities
            .contains(capability)
            .then_some(Ty::Bool)
            .ok_or_else(|| format!("native scalar lacks {capability} capability"));
    }
    let ty = match op {
        "+" if matches!(*a, Ty::Str | Ty::I64 | Ty::F64) => a.clone(),
        "/" if matches!(*a, Ty::I64 | Ty::F64) => Ty::F64,
        "-" | "*" | "%" if matches!(*a, Ty::I64 | Ty::F64) => a.clone(),
        ">" | "<" | ">=" | "<="
            if matches!(
                *a,
                Ty::I64
                    | Ty::F64
                    | Ty::Date
                    | Ty::DateTime
                    | Ty::CivilDateTime
                    | Ty::Time
                    | Ty::Duration
                    | Ty::Bytes
                    | Ty::Bool
                    | Ty::Any
            ) =>
        {
            Ty::Bool
        }
        "==" | "!=" if matches!(a, Ty::Family(_) | Ty::Set(_) | Ty::Map(..) | Ty::Tuple(_)) => {
            Ty::Bool
        }
        "==" | "!="
            if !matches!(
                a.name(),
                "void" | "set" | "struct" | "list" | "delta" | "map" | "tuple"
            ) =>
        {
            Ty::Bool
        }
        "&&" | "||" if *a == Ty::Bool => Ty::Bool,
        _ => return Err(format!("unsupported binary operation {op}")),
    };
    Ok(ty)
}
