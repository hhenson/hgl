//! Resolve list sizes through checked constant evaluation before type identity.
use hgl_rust_ir::{Kind, Value};
use hgl_source::{Cursor, Expr, Literal, Ty, application, delta_argument, lex};
/// Normalize each nested explicit list size using its lexical constant context.
pub fn normalize(
    name: &str,
    evaluate: &mut impl FnMut(&str) -> Result<Literal, String>,
) -> Result<String, String> {
    if let Some(origin) = delta_argument(name) {
        return Ok(format!("delta<{}>", normalize(origin, evaluate)?));
    }
    let Some((base, args)) = application(name) else {
        return Ok(name.into());
    };
    if base == "list" {
        if !(1..=2).contains(&args.len()) {
            return Err("list requires an element and optional constant size".into());
        }
        let element = normalize(args[0], evaluate)?;
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
    let args = args
        .into_iter()
        .map(|arg| normalize(arg, evaluate))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("{base}<{}>", args.join(",")))
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
    let value = hgl_value_eval::Evaluator::default()
        .value(&value)
        .map_err(|error| format!("constant list size: {error}"))?;
    let Kind::Literal(value) = value.kind else {
        return Err("list size requires a constant scalar".into());
    };
    Ok(value)
}
fn expression_value(expr: &Expr) -> Result<Value, String> {
    match expr {
        Expr::Literal(value) => Ok(Value::new(value.ty(), Kind::Literal(value.clone()))),
        Expr::Binary(op, a, b) => {
            let a = expression_value(a)?;
            let b = expression_value(b)?;
            let ty = hgl_value_check::binary_type(op, &a.ty, &b.ty)?;
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
        Expr::Name(_)
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
