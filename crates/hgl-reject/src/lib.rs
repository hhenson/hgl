//! Annotation-owned source probes for ordinary mixed HGL test runs.
use hgl_diagnostics::Diagnostic;
use hgl_test_annotations::{Expectation, annotations};
use hgl_test_units::{Unit, mask, units};
/// One checked rejection result; failure never prevents other valid tests from running.
#[derive(Debug)]
pub struct Outcome {
    /// Qualified test name or source/declaration-line identity.
    pub name: String,
    /// Exact diagnostic matching result.
    pub result: Result<(), String>,
}
#[derive(Debug)]
struct Case {
    source: usize,
    unit: Unit,
    expected: Vec<Expectation>,
    selected: bool,
}
/// Surviving source and independently restored rejection owners.
#[derive(Debug)]
pub struct Plan {
    /// Ordinary executable compilation unit with all rejection owners excluded.
    pub sources: Vec<(String, String)>,
    originals: Vec<(String, String)>,
    cases: Vec<Case>,
}
impl Plan {
    /// Validate metadata only in explicit target files/parts and isolate their owners.
    pub fn prepare(sources: Vec<(String, String)>, explicit: usize) -> Result<Self, String> {
        let mut cases: Vec<Case> = Vec::new();
        for (source, (file, text)) in sources.iter().enumerate().take(explicit) {
            let expected = annotations(text).map_err(|e| format!("{file}: {e}"))?;
            if expected.is_empty() {
                continue;
            }
            let units = units(text).map_err(|e| format!("{file}: {e}"))?;
            for expectation in expected {
                let unit = units
                    .iter()
                    .find(|u| {
                        u.line <= expectation.line
                            && expectation.line <= u.end_line
                            && (u.line != expectation.line || u.first_on_line)
                    })
                    .ok_or_else(|| {
                        format!(
                            "{file}:{}: orphaned expectation or invalid owner",
                            expectation.line
                        )
                    })?;
                if let Some(case) = cases
                    .iter_mut()
                    .find(|c| c.source == source && c.unit.span == unit.span)
                {
                    case.expected.push(expectation);
                } else {
                    cases.push(Case {
                        source,
                        unit: unit.clone(),
                        expected: vec![expectation],
                        selected: true,
                    });
                }
            }
        }
        let originals = sources.clone();
        let sources = sources
            .into_iter()
            .enumerate()
            .map(|(i, (name, text))| {
                let ranges = cases
                    .iter()
                    .filter(|c| c.source == i)
                    .map(|c| c.unit.span.clone());
                (name, mask(&text, ranges))
            })
            .collect();
        Ok(Self {
            sources,
            originals,
            cases,
        })
    }
    /// Apply the ordinary named-test selector rules across both kinds of case.
    pub fn select(&mut self, names: &[&str], executable: &[String]) -> Result<(), String> {
        let mut all = executable.to_vec();
        all.extend(self.cases.iter().filter_map(|c| c.unit.test_name()));
        let mut unique = std::collections::BTreeSet::new();
        for name in &all {
            if !unique.insert(name) {
                return Err(format!("duplicate test {name}"));
            }
        }
        for name in names {
            if !all.iter().any(|full| selected(full, &[*name])) {
                return Err(format!("unknown test selector {name}"));
            }
        }
        for case in &mut self.cases {
            case.selected = case
                .unit
                .test_name()
                .is_none_or(|name| selected(&name, names));
        }
        Ok(())
    }
    /// Check every required rejection independently against the ordinary compiler.
    pub fn check(&self) -> Vec<Outcome> {
        self.cases
            .iter()
            .filter(|c| c.selected)
            .map(|case| {
                let mut sources = self.sources.clone();
                let (file, original) = &self.originals[case.source];
                let span = case.unit.span.clone();
                sources[case.source]
                    .1
                    .replace_range(span.clone(), &original[span]);
                let errors = hgl_program::module_diagnostics(&sources);
                let name = case.unit.test_name().unwrap_or_else(|| {
                    format!(
                        "{file}:{}{}",
                        case.unit.line,
                        case.unit
                            .name
                            .as_ref()
                            .map_or(String::new(), |name| format!(" {name}"))
                    )
                });
                Outcome {
                    name,
                    result: match_errors(file, &case.expected, &errors),
                }
            })
            .collect()
    }
}
/// Match short or qualified test names, with no selector meaning every test.
pub fn selected(name: &str, names: &[&str]) -> bool {
    names.is_empty()
        || names
            .iter()
            .any(|selector| name == *selector || name.rsplit("::").next() == Some(*selector))
}
fn match_errors(
    source: &str,
    expected: &[Expectation],
    errors: &[Diagnostic],
) -> Result<(), String> {
    let mut matched = vec![false; expected.len()];
    let mut failures = Vec::new();
    for error in errors {
        let found = expected.iter().enumerate().find(|(index, expectation)| {
            !matched[*index]
                && error.source == source
                && expectation.line == error.line
                && expectation.category == error.issue.category
                && error.issue.code == Some(expectation.code.as_str())
        });
        if let Some((index, _)) = found {
            matched[index] = true;
        } else {
            failures.push(format!("unexpected {error}"));
        }
    }
    for (expectation, matched) in expected.iter().zip(matched) {
        if !matched {
            failures.push(format!(
                "{source}:{}: missing {}[{}]",
                expectation.line, expectation.category, expectation.code
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[cfg(test)]
#[path = "../tests/support/matching.rs"]
mod tests;
