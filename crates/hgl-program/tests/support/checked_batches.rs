//! Bound generated Rust compilation without repeating source admission.
use hgl_program::Suite;
use hgl_semantics::{harness_ir::Step, ir::Plan};
use std::collections::BTreeMap;

pub(super) fn split(suite: Suite, count: usize) -> Vec<Suite> {
    assert!(count > 0);
    let mut plans = suite.plans.into_iter().map(Some).collect::<Vec<_>>();
    let mut tests = suite.tests.into_iter();
    let mut batches = Vec::new();
    loop {
        let mut batch = Suite {
            tests: tests.by_ref().take(count).collect(),
            plans: Vec::new(),
            native_images: suite.native_images.clone(),
        };
        if batch.tests.is_empty() {
            break;
        }
        let mut cases = BTreeMap::new();
        for test in &mut batch.tests {
            reindex(&mut test.steps, &mut plans, &mut batch.plans, &mut cases);
        }
        batches.push(batch);
    }
    assert!(
        plans.iter().all(Option::is_none),
        "every checked graph must be retained"
    );
    batches
}
fn reindex(
    steps: &mut [Step],
    source: &mut [Option<Plan>],
    plans: &mut Vec<Plan>,
    cases: &mut BTreeMap<usize, usize>,
) {
    for step in steps {
        match step {
            Step::Eval(evaluation) | Step::BindEval(_, _, evaluation) => {
                evaluation.case = *cases.entry(evaluation.case).or_insert_with(|| {
                    let case = plans.len();
                    plans.push(
                        source[evaluation.case]
                            .take()
                            .unwrap_or_else(|| unreachable!("test-owned graph plan")),
                    );
                    case
                });
            }
            Step::If(_, yes, no) => {
                reindex(yes, source, plans, cases);
                reindex(no, source, plans, cases);
            }
            Step::Raises(_, body) => reindex(body, source, plans, cases),
            Step::Ordinary(_) | Step::Assert(_) => {}
        }
    }
}
