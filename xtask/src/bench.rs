//! `cargo xtask bench`: run one benchmark program several times, each in a
//! fresh process, and report the median and spread of what it measured.
//!
//! The program must print one line containing `"ns_per_cycle":<number>` and
//! `"ok":true`. Both halves of a scenario pair -- the C++ baseline and its
//! Rust twin -- are measured through this one command, so they are measured
//! the same way (docs/decisions/0002).

use std::process::Command;

const USAGE: &str = "usage: cargo xtask bench [--samples N] [--pin CORE] -- PROGRAM [ARGS...]";
const MEASURE: &str = "\"ns_per_cycle\":";

/// What one `bench` invocation was asked to do.
struct Request {
    samples: usize,
    /// A CPU core to pin every sample to, through `taskset`. Linux only.
    pin: Option<String>,
    command: Vec<String>,
}

/// The spread of one scenario's samples, in nanoseconds per cycle.
struct Summary {
    median: f64,
    /// Median absolute deviation from the median.
    mad: f64,
    min: f64,
    max: f64,
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let request = parse(args)?;
    let mut samples = Vec::with_capacity(request.samples);
    for _ in 0..request.samples {
        samples.push(sample(&request)?);
    }
    let summary = summarise(&samples).ok_or("no samples were taken")?;
    println!(
        "{} samples  median {:.2} ns/cycle  mad {:.2} ({:.2}%)  min {:.2}  max {:.2}",
        samples.len(),
        summary.median,
        summary.mad,
        100.0 * summary.mad / summary.median,
        summary.min,
        summary.max,
    );
    println!("{}", request.command.join(" "));
    Ok(())
}

fn parse(args: &[String]) -> Result<Request, String> {
    let mut request = Request {
        samples: 15,
        pin: None,
        command: Vec::new(),
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--samples" => {
                let text = rest.next().ok_or(USAGE)?;
                request.samples = text
                    .parse()
                    .map_err(|error| format!("--samples {text}: {error}"))?;
            }
            "--pin" => request.pin = Some(rest.next().ok_or(USAGE)?.clone()),
            "--" => {
                request.command = rest.cloned().collect();
                break;
            }
            other => return Err(format!("unexpected argument '{other}'\n{USAGE}")),
        }
    }
    if request.command.is_empty() || request.samples == 0 {
        return Err(USAGE.to_owned());
    }
    Ok(request)
}

/// Run the program once and return the nanoseconds per cycle it reported.
fn sample(request: &Request) -> Result<f64, String> {
    let mut words: Vec<&str> = Vec::new();
    if let Some(core) = &request.pin {
        words.extend(["taskset", "-c", core.as_str()]);
    }
    words.extend(request.command.iter().map(String::as_str));
    let (program, arguments) = words.split_first().ok_or(USAGE)?;

    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || !text.contains("\"ok\":true") {
        return Err(format!("the run did not pass its own check:\n{text}"));
    }
    measured(&text).ok_or_else(|| format!("no {MEASURE} in the output:\n{text}"))
}

fn measured(text: &str) -> Option<f64> {
    let after = text.split_once(MEASURE)?.1;
    let number = after.split([',', '}']).next()?;
    number.trim().parse().ok()
}

fn summarise(samples: &[f64]) -> Option<Summary> {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = median_of(&sorted)?;
    let mut deviations: Vec<f64> = sorted.iter().map(|value| (value - median).abs()).collect();
    deviations.sort_by(f64::total_cmp);
    Some(Summary {
        median,
        mad: median_of(&deviations)?,
        min: *sorted.first()?,
        max: *sorted.last()?,
    })
}

/// The median of values already in ascending order.
fn median_of(sorted: &[f64]) -> Option<f64> {
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        let below = sorted.get(middle.checked_sub(1)?)?;
        Some(f64::midpoint(*below, *sorted.get(middle)?))
    } else {
        sorted.get(middle).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{measured, summarise};

    #[test]
    fn reads_the_measurement_from_a_result_line() {
        let line = r#"{"scenario":"tick","ns_per_cycle":306.54,"ok":true}"#;
        assert_eq!(measured(line), Some(306.54));
        assert_eq!(measured(r#"{"ok":true}"#), None);
    }

    #[test]
    fn summarises_odd_and_even_sample_counts() {
        let odd = summarise(&[5.0, 1.0, 3.0]).unwrap();
        assert_eq!(
            (odd.median, odd.mad, odd.min, odd.max),
            (3.0, 2.0, 1.0, 5.0)
        );
        let even = summarise(&[4.0, 1.0, 3.0, 2.0]).unwrap();
        assert_eq!((even.median, even.mad), (2.5, 1.0));
        assert!(summarise(&[]).is_none());
    }
}
