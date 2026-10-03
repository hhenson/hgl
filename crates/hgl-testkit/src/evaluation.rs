//! HGL evaluation with sparse recording and dense sequence comparison.
use hgl_types::EngineTime;

/// Sparse storage of a dense result, including its trailing silent cells.
#[derive(Debug)]
pub struct Observation<T> {
    length: usize,
    ticks: Vec<(usize, T)>,
}

/// Convert independently owned capture ticks into a sparse dense observation.
/// The horizon comes from inputs and actual captures, never expectations.
pub fn observe<T>(
    ticks: Vec<(EngineTime, T)>,
    input_length: usize,
) -> Result<Observation<T>, String> {
    if ticks.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err("eval recording timestamps did not advance".into());
    }
    let ticks = ticks
        .into_iter()
        .map(|(time, value)| {
            let cycle = time
                .micros()
                .checked_sub(EngineTime::MIN_START.micros())
                .ok_or("eval timestamp overflow")?;
            Ok((usize::try_from(cycle).map_err(|e| e.to_string())?, value))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let last = ticks.last().map_or(Ok(0), |(i, _)| {
        i.checked_add(1).ok_or("eval horizon overflow")
    })?;
    Ok(Observation {
        length: input_length.max(last),
        ticks,
    })
}

/// Compare equal-length dense sequences, reporting the first differing cycle.
pub fn compare<T: PartialEq + std::fmt::Debug>(
    expected: &[Option<T>],
    observed: &Observation<T>,
) -> Result<(), String> {
    let mut ticks = observed.ticks.iter().peekable();
    for (cycle, wanted) in expected.iter().take(observed.length).enumerate() {
        let actual = if ticks.peek().is_some_and(|(i, _)| *i == cycle) {
            ticks.next().map(|(_, value)| value)
        } else {
            None
        };
        if wanted.as_ref() != actual {
            return Err(format!(
                "cycle {cycle}: expected {wanted:?}, observed {actual:?}"
            ));
        }
    }
    if expected.len() != observed.length {
        return Err(format!(
            "cycle {}: expected length {}, observed length {}",
            expected.len().min(observed.length),
            expected.len(),
            observed.length
        ));
    }
    Ok(())
}
