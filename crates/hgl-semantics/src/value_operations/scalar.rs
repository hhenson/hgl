use crate::ir::{Kind, Value};
use crate::value_operations::{EvalError, unsupported};
use hgl_source::Literal;
use std::cmp::Ordering;

fn scalar(value: &Value) -> Result<&Literal, EvalError> {
    if let Kind::Literal(value) = &value.kind {
        Ok(value)
    } else {
        Err(unsupported("scalar operator requires literal operands"))
    }
}
fn value(literal: Literal) -> Value {
    Value::new(literal.ty(), Kind::Literal(literal))
}
fn overflow() -> EvalError {
    unsupported("integer overflow policy is not specified")
}
/// Evaluate a checked ordinary scalar unary operation.
pub fn unary(op: &str, operand: &Value) -> Result<Value, EvalError> {
    let result = match (op, scalar(operand)?) {
        ("!", Literal::Bool(v)) => Literal::Bool(!v),
        ("-", Literal::Int(v)) => Literal::Int(v.checked_neg().ok_or_else(overflow)?),
        ("-", Literal::Float(v)) => Literal::Float(-v),
        ("-", Literal::Duration(v)) => Literal::Duration(
            v.checked_neg()
                .ok_or_else(|| EvalError::Operation("time arithmetic overflow".into()))?,
        ),
        ("float", Literal::Int(v)) => Literal::Float(as_float(*v)),
        _ => return Err(unsupported("unsupported ordinary unary operation")),
    };
    Ok(value(result))
}
#[expect(
    clippy::cast_precision_loss,
    reason = "HGL admits ordinary i64 to f64 widening"
)]
fn as_float(value: i64) -> f64 {
    value as f64
}
/// Evaluate a checked ordinary scalar or family equality operation.
pub fn binary(op: &str, a: &Value, b: &Value) -> Result<Value, EvalError> {
    if matches!(
        a.ty,
        hgl_source::Ty::Set(_) | hgl_source::Ty::Map(..) | hgl_source::Ty::Family(_)
    ) && matches!(op, "==" | "!=")
    {
        return Ok(value(Literal::Bool(
            crate::collection_values::equal(a, b) == (op == "=="),
        )));
    }
    let (a, b) = (scalar(a)?, scalar(b)?);
    if a.ty() == b.ty() && matches!(op, "==" | "!=") && !matches!(a, Literal::Float(_)) {
        return Ok(value(Literal::Bool((a == b) == (op == "=="))));
    }
    let result = match (a, b) {
        (Literal::Int(a), Literal::Int(b)) => integer(op, *a, *b)?,
        (Literal::Float(a), Literal::Float(b)) => floating(op, *a, *b)?,
        (Literal::Bool(a), Literal::Bool(b)) => Literal::Bool(match op {
            "&&" => *a && *b,
            "||" => *a || *b,
            "==" => a == b,
            "!=" => a != b,
            _ => return Err(unsupported("unsupported ordinary bool operation")),
        }),
        (Literal::Str(a), Literal::Str(b)) if op == "+" => Literal::Str(format!("{a}{b}")),
        (Literal::Str(a), Literal::Str(b)) => comparison(op, Some(a.cmp(b)))?,
        (Literal::Duration(a), Literal::Duration(b)) if matches!(op, "+" | "-") => {
            Literal::Duration(time_arithmetic(op, *a, *b)?)
        }
        (Literal::DateTime(a), Literal::Duration(b)) if matches!(op, "+" | "-") => {
            Literal::DateTime(time_arithmetic(op, *a, *b)?)
        }
        (Literal::Duration(a), Literal::DateTime(b)) if op == "+" => {
            Literal::DateTime(time_arithmetic(op, *a, *b)?)
        }
        (Literal::DateTime(a), Literal::DateTime(b)) if op == "-" => {
            Literal::Duration(time_arithmetic(op, *a, *b)?)
        }
        (Literal::Duration(a), Literal::Duration(b))
        | (Literal::Date(a), Literal::Date(b))
        | (Literal::CivilDateTime(a), Literal::CivilDateTime(b))
        | (Literal::Time(a), Literal::Time(b))
        | (Literal::DateTime(a), Literal::DateTime(b)) => comparison(op, Some(a.cmp(b)))?,
        _ => return Err(unsupported("unsupported ordinary scalar operand types")),
    };
    Ok(value(result))
}
fn integer(op: &str, a: i64, b: i64) -> Result<Literal, EvalError> {
    let result = match op {
        "+" => a.checked_add(b),
        "-" => a.checked_sub(b),
        "*" => a.checked_mul(b),
        "/" => return floating(op, as_float(a), as_float(b)),
        "%" | "//" => {
            if b == 0 {
                return Err(EvalError::Operation("division by zero".into()));
            }
            if op == "%" && b == -1 {
                return Ok(Literal::Int(0));
            }
            let quotient = a.checked_div(b).ok_or_else(overflow)?;
            let remainder = a.checked_rem(b).ok_or_else(overflow)?;
            let adjust = remainder != 0 && (remainder < 0) != (b < 0);
            Some(if op == "//" {
                quotient - i64::from(adjust)
            } else if adjust {
                remainder + b
            } else {
                remainder
            })
        }
        _ => return comparison(op, Some(a.cmp(&b))),
    };
    result.map(Literal::Int).ok_or_else(overflow)
}
fn floating(op: &str, a: f64, b: f64) -> Result<Literal, EvalError> {
    if matches!(op, "/" | "//" | "%") && b == 0.0 {
        return Err(EvalError::Operation("division by zero".into()));
    }
    let result = match op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" => a / b,
        "//" => (a / b).floor(),
        "%" => {
            let remainder = a % b;
            if remainder == 0.0 {
                0.0_f64.copysign(b)
            } else if (remainder < 0.0) != (b < 0.0) {
                remainder + b
            } else {
                remainder
            }
        }
        _ => return comparison(op, a.partial_cmp(&b)),
    };
    Ok(Literal::Float(result))
}
fn comparison(op: &str, ordering: Option<Ordering>) -> Result<Literal, EvalError> {
    let ordering = ordering.ok_or_else(|| unsupported("NaN comparison policy is not specified"))?;
    Ok(Literal::Bool(match op {
        "==" => ordering.is_eq(),
        "!=" => !ordering.is_eq(),
        "<" => ordering.is_lt(),
        "<=" => !ordering.is_gt(),
        ">" => ordering.is_gt(),
        ">=" => !ordering.is_lt(),
        _ => return Err(unsupported("unsupported ordinary binary operation")),
    }))
}

fn time_arithmetic(op: &str, a: i64, b: i64) -> Result<i64, EvalError> {
    let result = if op == "+" {
        a.checked_add(b)
    } else {
        a.checked_sub(b)
    };
    result.ok_or_else(|| EvalError::Operation("time arithmetic overflow".into()))
}
