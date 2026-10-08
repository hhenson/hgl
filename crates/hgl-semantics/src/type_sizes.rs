//! Resolve list sizes through checked constant evaluation before type identity.
use crate::ir::{Kind, Value};
use hgl_source::{Cursor, Expr, Issue, Literal, Ty, application, delta_argument, lex};
/// Normalize each nested explicit list size using its lexical constant context.
pub fn normalize(
    name: &str,
    evaluate: &mut impl FnMut(&str) -> Result<Literal, String>,
) -> Result<String, String> {
    normalize_checked(name, &mut |expr| evaluate(expr).map_err(Issue::from)).map_err(String::from)
}
/// Normalize sizes while retaining their catalogued rule and relative argument range.
pub fn normalize_checked(
    name: &str,
    evaluate: &mut impl FnMut(&str) -> Result<Literal, Issue>,
) -> Result<String, Issue> {
    if let Some(origin) = delta_argument(name) {
        return Ok(format!(
            "delta<{}>",
            normalize_checked(origin, evaluate).map_err(|e| e.shifted(6))?
        ));
    }
    let Some((base, args)) = application(name) else {
        return Ok(name.into());
    };
    if base == "rolling" {
        return rolling(base, &args, evaluate);
    }
    if base == "list" {
        if !(1..=2).contains(&args.len()) {
            return Err("list requires an element and optional constant size".into());
        }
        let element =
            normalize_checked(args[0], evaluate).map_err(|e| e.shifted(base.len() + 1))?;
        if args.len() == 1 || args[1] == "unbounded" {
            return Ok(format!("list<{element}>"));
        }
        let Literal::Int(size) = evaluate(args[1])? else {
            return Err("list size requires a constant i64".into());
        };
        if size < 0 {
            return Err("list size must be nonnegative or unbounded".into());
        }
        return Ok(format!("list<{element},{size}>"));
    }
    let mut offset = base.len() + 1;
    let args = args
        .into_iter()
        .map(|arg| {
            let result = normalize_checked(arg, evaluate).map_err(|e| e.shifted(offset));
            offset += arg.len() + 1;
            result
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{base}<{}>", args.join(",")))
}
/// Collect explicit bound expressions without evaluating or fabricating size values.
pub fn expressions(name: &str) -> Vec<&str> {
    if let Some(origin) = delta_argument(name) {
        return expressions(origin);
    }
    let Some((base, args)) = application(name) else {
        return Vec::new();
    };
    let bounded = matches!(base, "list" | "rolling");
    let mut bounds = args
        .iter()
        .take(if bounded { 1 } else { args.len() })
        .flat_map(|arg| expressions(arg))
        .collect::<Vec<_>>();
    if bounded {
        bounds.extend(args.into_iter().skip(1).filter(|arg| *arg != "unbounded"));
    }
    bounds
}
/// Evaluate a closed scalar size expression with ordinary checked semantics.
pub fn literal(source: &str) -> Result<Literal, String> {
    let tokens = lex(source)?;
    let mut cursor = Cursor::new(&tokens);
    let expression = cursor.expr()?;
    if !cursor.at("") {
        return Err("invalid list-size expression".into());
    }
    let value = expression_value(&expression)?;
    let value = crate::value_eval::Evaluator::default()
        .value(&value)
        .map_err(|error| format!("constant list size: {error}"))?;
    let Kind::Literal(value) = value.kind else {
        return Err("list size requires a constant scalar".into());
    };
    Ok(value)
}
fn expression_value(expr: &Expr) -> Result<Value, String> {
    match expr.syntax() {
        Expr::Located(..) => unreachable!("syntax strips source origins"),
        Expr::Literal(value) => Ok(Value::new(value.ty(), Kind::Literal(value.clone()))),
        Expr::Binary(op, a, b) => {
            let a = expression_value(a)?;
            let b = expression_value(b)?;
            let ty = crate::value_check::binary_type(op, &a.ty, &b.ty)?;
            Ok(Value::new(
                ty,
                Kind::Binary(op.clone(), Box::new(a), Box::new(b)),
            ))
        }
        Expr::Unary(op, expr) => {
            let value = expression_value(expr)?;
            if !matches!(
                (op.as_str(), &value.ty),
                ("-", Ty::I64 | Ty::F64 | Ty::Duration) | ("!", Ty::Bool)
            ) {
                return Err("invalid constant unary operation".into());
            }
            Ok(Value::new(
                value.ty.clone(),
                Kind::Unary(op.clone(), Box::new(value)),
            ))
        }
        Expr::Lambda(..)
        | Expr::Name(_)
        | Expr::Call(..)
        | Expr::Applied(..)
        | Expr::Property(..)
        | Expr::Index(..)
        | Expr::Sequence(_)
        | Expr::Sparse(_)
        | Expr::Tuple(_)
        | Expr::TemporalLiteral(_)
        | Expr::Null => Err("list size requires a resolved constant expression".into()),
    }
}

fn rolling(
    base: &str,
    args: &[&str],
    evaluate: &mut impl FnMut(&str) -> Result<Literal, Issue>,
) -> Result<String, Issue> {
    if !(2..=3).contains(&args.len()) {
        return Err("rolling requires payload, maximum and optional minimum".into());
    }
    let max = evaluate(args[1])?;
    let min = if args.len() == 3 {
        evaluate(args[2])?
    } else {
        max.clone()
    };
    let argument = if matches!(max, Literal::Int(_) | Literal::Duration(_)) {
        2.min(args.len() - 1)
    } else {
        1
    };
    let offset = base.len() + 1 + args[..argument].iter().map(|a| a.len() + 1).sum::<usize>();
    let span = offset..offset + args[argument].len();
    let (kind, max, min) = match (max, min) {
        (Literal::Int(max), Literal::Int(min)) => (hgl_source::WindowKind::Ticks, max, min),
        (Literal::Duration(max), Literal::Duration(min)) => {
            (hgl_source::WindowKind::Duration, max, min)
        }
        _ => {
            return Err(Issue::coded(
                "type",
                "rolling.size_kind",
                span,
                "rolling sizes require constants of the same i64 or duration kind",
            ));
        }
    };
    let window = hgl_source::Window::new(kind, max, min).map_err(|message| {
        let span = if max <= 0 {
            let start = base.len() + 1 + args[0].len() + 1;
            start..start + args[1].len()
        } else {
            span
        };
        Issue::coded("type", "rolling.size_bounds", span, message)
    })?;
    Ok(format!(
        "rolling<{},{}>",
        normalize_checked(args[0], evaluate).map_err(|e| e.shifted(base.len() + 1))?,
        window.source_name()
    ))
}
