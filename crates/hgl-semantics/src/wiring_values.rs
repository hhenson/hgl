//! Cold execution is deferred as checked data; topology remains checker-owned.
use crate::ir::{Kind, Plan, Statement, Value};
use crate::value_eval::{EvalError, Evaluator};
use hgl_source::Issue;

fn result<T>(
    plan: &mut Plan,
    result: Result<T, EvalError>,
    failed: impl FnOnce(String) -> T,
) -> Result<T, Issue> {
    match result {
        Ok(value) => Ok(value),
        Err(EvalError::Operation(message)) => {
            if plan.ordinary_instantiation && !plan.wiring.is_empty() {
                plan.wiring.push(Value::new(
                    hgl_source::Ty::Void,
                    Kind::WiringFailure(message.clone()),
                ));
            } else {
                plan.construction_error.get_or_insert(message.clone());
            }
            Ok(failed(message))
        }
        Err(error) => Err(error.to_string().into()),
    }
}
fn substitute(evaluator: &mut Evaluator, value: &mut Value) -> Result<(), EvalError> {
    let mut error = None;
    crate::ir_visit::value_mut(value, &mut |v| {
        if let Kind::ValueCall(args, _) = &mut v.kind {
            for arg in args {
                if let Err(e) = substitute(evaluator, arg) {
                    error = Some(e);
                }
            }
            return false;
        }
        if matches!(v.kind, Kind::Local(_) | Kind::MutableLocal(_)) {
            match evaluator.value(v) {
                Ok(bound) => *v = bound,
                Err(e) => error = Some(e),
            }
            return false;
        }
        true
    });
    error.map_or(Ok(()), Err)
}
/// Resolve lexical values, retaining provider-dependent construction for execution.
pub fn value(evaluator: &mut Evaluator, plan: &mut Plan, input: &Value) -> Result<Value, Issue> {
    let mut recipe = input.clone();
    if plan.ordinary_instantiation {
        if let Err(error) = substitute(evaluator, &mut recipe) {
            return result(plan, Err(error), |message| {
                Value::new(input.ty.clone(), Kind::WiringFailure(message))
            });
        }
        if matches!(recipe.kind, Kind::Prepared(_)) {
            return Ok(recipe);
        }
        let mut deferred = false;
        crate::ir_visit::value(&recipe, &mut |v| {
            deferred |= matches!(&v.kind, Kind::Prepared(_))
                || matches!(&v.kind, Kind::Native(n, _) if n.provider.is_some());
            true
        });
        if deferred {
            let id = plan.wiring.len();
            plan.wiring.push(recipe);
            return Ok(Value::new(input.ty.clone(), Kind::Prepared(id)));
        }
    }
    let evaluated = evaluator.value(input);
    result(plan, evaluated, |message| {
        Value::new(input.ty.clone(), Kind::WiringFailure(message))
    })
}
/// Apply ordinary lexical writes while preserving deferred recipe identity and order.
pub fn statement(
    evaluator: &mut Evaluator,
    plan: &mut Plan,
    input: &Statement,
) -> Result<(), Issue> {
    if plan.ordinary_instantiation {
        match input {
            Statement::Let(id, v) | Statement::Var(id, v) => {
                let v = value(evaluator, plan, v)?;
                evaluator.bind_checked(*id, v, matches!(input, Statement::Var(..)));
                return Ok(());
            }
            Statement::Assign(target, v) => {
                let v = value(evaluator, plan, v)?;
                let local = Value::new(v.ty.clone(), Kind::Local(usize::MAX));
                let applied = evaluator.scoped(|scope| {
                    scope.bind_checked(usize::MAX, v, false);
                    scope.statement(&Statement::Assign(target.clone(), local))
                });
                return result(plan, applied, |_| None).map(|_| ());
            }
            Statement::If(condition, yes, no) => {
                let condition = value(evaluator, plan, condition)?;
                let Kind::Literal(hgl_source::Literal::Bool(selected)) = condition.kind else {
                    return Err("provider-dependent wiring topology is unsupported".into());
                };
                return evaluator.scoped(|scope| {
                    for s in if selected { yes } else { no } {
                        statement(scope, plan, s)?;
                    }
                    Ok(())
                });
            }
            Statement::Call(v) => {
                value(evaluator, plan, v)?;
                return Ok(());
            }
            Statement::ForItems(..)
            | Statement::Exit
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::While(..)
            | Statement::Yield(_)
            | Statement::For(..) => {}
        }
    }
    let applied = evaluator.statement(input);
    result(plan, applied, |_| None).map(|_| ())
}
