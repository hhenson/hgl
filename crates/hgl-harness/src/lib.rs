//! Execute checked ordinary setup before and between independent graph runs.
use hgl_harness_ir::{Argument, CapturedEval, PreparedEval, Step, Test};
use hgl_rust_ir::{DeltaEntry, Kind, Value};
use hgl_source::{Literal, TemporalLiteral};
use hgl_value_eval::{EvalError, Evaluator};

/// Execute a lexical test once, retaining owning locals across fresh evals.
pub fn execute(
    test: &Test,
    mut materialize: impl FnMut(&TemporalLiteral) -> Result<Literal, EvalError>,
    mut eval: impl FnMut(usize, PreparedEval) -> Result<CapturedEval, String>,
) -> Result<usize, String> {
    let mut evaluator = Evaluator::default();
    let mut count = 0;
    for step in &test.steps {
        match step {
            Step::Ordinary(statement) => {
                if evaluator
                    .statement_with(statement, &mut materialize)
                    .map_err(|e| e.to_string())?
                    .is_some()
                {
                    return Err("test setup cannot return a value".into());
                }
            }
            Step::Assert(value) => {
                let value = evaluator
                    .value_with(value, &mut materialize)
                    .map_err(|e| e.to_string())?;
                if !matches!(value.kind, Kind::Literal(Literal::Bool(true))) {
                    return Err("ordinary assertion failed".into());
                }
                count += 1;
            }
            Step::Eval(evaluation) => {
                let prepared = prepare(&mut evaluator, &evaluation.arguments, &mut materialize)?;
                let actual = eval(evaluation.case, prepared)?;
                if let Some(expected) = &evaluation.expected {
                    let expected = expected
                        .iter()
                        .map(|slot| {
                            slot.as_ref()
                                .map(|value| evaluator.value_with(value, &mut materialize))
                                .transpose()
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| e.to_string())?;
                    compare(&expected, &actual)?;
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
    materialize: &mut impl FnMut(&TemporalLiteral) -> Result<Literal, EvalError>,
) -> Result<PreparedEval, String> {
    let mut values = vec![None; arguments.len()];
    let mut input_length = 0;
    for argument in arguments {
        let (binding, value) = match argument {
            Argument::Constant { binding, value } => (
                *binding,
                evaluator
                    .value_with(value, materialize)
                    .map_err(|e| e.to_string())?,
            ),
            Argument::Dense {
                parameter,
                binding,
                shape,
                entry_type,
                slots,
            } => {
                let slots = slots
                    .iter()
                    .map(|slot| {
                        slot.as_ref()
                            .map(|value| evaluator.value_with(value, materialize))
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                hgl_eval_data::validate(shape, &slots).map_err(|(index, error)| {
                    format!("eval: input delta outside publication profile: {parameter} at position {index}: {error}")
                })?;
                input_length = input_length.max(slots.len());
                (*binding, hgl_eval_data::timed(entry_type.clone(), &slots)?)
            }
        };
        let target = values.get_mut(binding).ok_or("invalid prepared binding")?;
        if target.replace(value).is_some() {
            return Err("duplicate prepared binding".into());
        }
    }
    let arguments = values
        .into_iter()
        .map(|value| value.ok_or("missing prepared binding".into()))
        .collect::<Result<_, String>>()?;
    Ok(PreparedEval {
        arguments,
        input_length,
    })
}
fn compare(expected: &[Option<Value>], actual: &CapturedEval) -> Result<(), String> {
    let mut ticks = actual.ticks.iter().peekable();
    for (index, wanted) in expected.iter().take(actual.length).enumerate() {
        let found = if ticks.peek().is_some_and(|(cycle, _)| *cycle == index) {
            ticks.next().map(|(_, v)| v)
        } else {
            None
        };
        if !match (wanted.as_ref(), found) {
            (Some(a), Some(b)) => equal(a, b),
            (None, None) => true,
            _ => false,
        } {
            return Err(format!(
                "cycle {index}: payload or presence differs; expected {wanted:?}, observed {found:?}"
            ));
        }
    }
    if expected.len() != actual.length {
        return Err(format!(
            "cycle {}: expected length {}, observed length {}",
            expected.len().min(actual.length),
            expected.len(),
            actual.length
        ));
    }
    Ok(())
}
fn equal(a: &Value, b: &Value) -> bool {
    if a.ty != b.ty {
        return false;
    }
    match (&a.kind, &b.kind) {
        (Kind::Literal(a), Kind::Literal(b)) => a == b,
        (Kind::List(a), Kind::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Kind::Construct(a), Kind::Construct(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(id, a)| b.iter().any(|(other, b)| id == other && equal(a, b)))
        }
        (Kind::Delta(a), Kind::Delta(b)) => {
            a.len() == b.len()
                && a.iter().all(|a| {
                    b.iter().any(|b| match (a, b) {
                        (DeltaEntry::Add(a), DeltaEntry::Add(b))
                        | (DeltaEntry::Remove(a), DeltaEntry::Remove(b)) => a == b,
                        (DeltaEntry::Child(i, a), DeltaEntry::Child(j, b)) => i == j && equal(a, b),
                        _ => false,
                    })
                })
        }
        _ => false,
    }
}
