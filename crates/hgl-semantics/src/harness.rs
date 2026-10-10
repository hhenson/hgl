//! Execute checked ordinary setup before and between independent graph runs.
use crate::harness_ir::{Argument, CapturedEval, Failure, PreparedEval, Step, Test};
use crate::ir::{Kind, Statement, Value};
use crate::value_eval::{EvalError, Evaluator};
use hgl_source::{Literal, TemporalLiteral};

/// Execute with the existing temporal-only materialization callback.
pub fn execute(
    test: &Test,
    materialize: impl FnMut(&TemporalLiteral) -> Result<Literal, EvalError>,
    eval: impl FnMut(usize, PreparedEval) -> Result<CapturedEval, Failure>,
) -> Result<usize, String> {
    execute_in_context(test, materialize, eval)
}
/// Execute a lexical test once, retaining owning locals across fresh evals.
pub fn execute_in_context(
    test: &Test,
    mut materialize: impl crate::value_eval::ExecutionContext<Value, crate::ir::Native, EvalError>,
    mut eval: impl FnMut(usize, PreparedEval) -> Result<CapturedEval, Failure>,
) -> Result<usize, String> {
    steps(
        &test.steps,
        &mut Evaluator::default(),
        &mut materialize,
        &mut eval,
    )
    .map_err(|error| error.to_string())
}
fn steps(
    body: &[Step],
    evaluator: &mut Evaluator,
    materialize: &mut impl crate::value_eval::ExecutionContext<Value, crate::ir::Native, EvalError>,
    eval: &mut impl FnMut(usize, PreparedEval) -> Result<CapturedEval, Failure>,
) -> Result<usize, Failure> {
    let mut count = 0;
    for step in body {
        match step {
            Step::Raises(code, body) => {
                let outcome =
                    evaluator.scoped(|evaluator| steps(body, evaluator, materialize, eval));
                match outcome {
                    Err(error) if error.matches(code) => count += 1,
                    Err(error) => return Err(format!("expected {code}: {error}").into()),
                    Ok(_) => {
                        return Err(format!("expected {code}, block completed normally").into());
                    }
                }
            }
            Step::Ordinary(statement) => {
                if evaluator
                    .statement_in_context(statement, materialize)
                    .map_err(Failure::from)?
                    .is_some()
                {
                    return Err("test setup cannot return a value".into());
                }
            }
            Step::Assert(value) => {
                let value = evaluator
                    .value_in_context(value, materialize)
                    .map_err(Failure::from)?;
                if !matches!(value.kind, Kind::Literal(Literal::Bool(true))) {
                    return Err("ordinary assertion failed".into());
                }
                count += 1;
            }
            Step::If(condition, yes, no) => {
                let value = evaluator
                    .value_in_context(condition, materialize)
                    .map_err(Failure::from)?;
                let Kind::Literal(Literal::Bool(condition)) = value.kind else {
                    return Err("test condition requires bool".into());
                };
                count += evaluator.scoped(|evaluator| {
                    steps(
                        if condition { yes } else { no },
                        evaluator,
                        materialize,
                        eval,
                    )
                })?;
            }
            Step::Eval(evaluation) | Step::BindEval(_, _, evaluation) => {
                let prepared = prepare(evaluator, &evaluation.arguments, materialize)?;
                let actual = eval(evaluation.case, prepared)?;
                if let Some(expected) = &evaluation.expected {
                    let expected = expected
                        .iter()
                        .map(|slot| {
                            slot.as_ref()
                                .map(|value| evaluator.value_in_context(value, materialize))
                                .transpose()
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(Failure::from)?;
                    compare(&expected, &actual)?;
                }
                if let Step::BindEval(id, ty, _) = step {
                    let retained =
                        Value::new(ty.clone(), Kind::Captured(actual.length, actual.ticks));
                    evaluator
                        .statement_in_context(&Statement::Let(*id, retained), materialize)
                        .map_err(Failure::from)?;
                }
                count += 1;
            }
        }
    }
    Ok(count)
}
fn prepare(
    evaluator: &mut Evaluator,
    arguments: &[Argument],
    materialize: &mut impl crate::value_eval::ExecutionContext<Value, crate::ir::Native, EvalError>,
) -> Result<PreparedEval, Failure> {
    let mut values = vec![None; arguments.len()];
    let mut dense = Vec::new();
    let mut input_length = 0;
    for argument in arguments {
        let (binding, value) = match argument {
            Argument::Constant { binding, value } => (
                *binding,
                evaluator
                    .value_in_context(value, materialize)
                    .map_err(Failure::from)?,
            ),
            Argument::Dense {
                parameter,
                binding,
                shape,
                entry_type,
                slots,
                sequence,
            } => {
                let slots = if let Some(sequence) = sequence {
                    let value = evaluator
                        .value_in_context(sequence, materialize)
                        .map_err(Failure::from)?;
                    let Kind::List(values) = value.kind else {
                        return Err("ordinary publication sequence requires a list".into());
                    };
                    values.into_iter().map(Some).collect()
                } else {
                    slots
                        .iter()
                        .map(|slot| {
                            slot.as_ref()
                                .map(|value| evaluator.value_in_context(value, materialize))
                                .transpose()
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(Failure::from)?
                };
                input_length = input_length.max(slots.len());
                let value = crate::eval_data::timed(entry_type.clone(), &slots)?;
                dense.push((parameter, shape, slots));
                (*binding, value)
            }
        };
        let target = values.get_mut(binding).ok_or("invalid prepared binding")?;
        if target.replace(value).is_some() {
            return Err("duplicate prepared binding".into());
        }
    }
    for (parameter, shape, slots) in dense {
        crate::eval_data::validate(shape, &slots).map_err(|(index, error)| {
            Failure::Execution(hgl_types::node_error::NodeError::coded(
                format!("eval: input delta outside publication profile: {parameter} at position {index}: {error}"),
                "eval.input_delta_profile",
            ))
        })?;
    }
    let arguments = values
        .into_iter()
        .map(|value| value.ok_or_else(|| "missing prepared binding".to_owned()))
        .collect::<Result<_, String>>()?;
    Ok(PreparedEval {
        arguments,
        input_length,
    })
}
fn compare(expected: &[Option<Value>], actual: &CapturedEval) -> Result<(), Failure> {
    let mut ticks = actual.ticks.iter().peekable();
    for (index, wanted) in expected.iter().take(actual.length).enumerate() {
        let found = if ticks.peek().is_some_and(|(cycle, _)| *cycle == index) {
            ticks.next().map(|(_, v)| v)
        } else {
            None
        };
        if !match (wanted.as_ref(), found) {
            (Some(a), Some(b)) => crate::any_values::equal_data(a, b).map_err(Failure::from)?,
            (None, None) => true,
            _ => false,
        } {
            return Err(format!(
                "cycle {index}: payload or presence differs; expected {wanted:?}, observed {found:?}"
            ).into());
        }
    }
    if expected.len() != actual.length {
        return Err(format!(
            "cycle {}: expected length {}, observed length {}",
            expected.len().min(actual.length),
            expected.len(),
            actual.length
        )
        .into());
    }
    Ok(())
}
