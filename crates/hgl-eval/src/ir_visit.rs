//! Harness traversal extends the syntax-neutral Model walkers.
use crate::ir::Value;
pub use hgl_value_model::ir_visit::*;

/// Visit every checked harness operand, including cold setup, graph inputs and expectations.
pub fn harness(tests: &[crate::harness_ir::Test], visit: &mut impl FnMut(&Value) -> bool) {
    use crate::harness_ir::{Argument, Evaluation, Step};
    fn evaluation(e: &Evaluation, visit: &mut impl FnMut(&Value) -> bool) {
        for argument in &e.arguments {
            match argument {
                Argument::Constant { value: v, .. } => value(v, visit),
                Argument::Dense {
                    slots, sequence, ..
                } => {
                    for v in slots
                        .iter()
                        .flatten()
                        .chain(sequence.iter().map(Box::as_ref))
                    {
                        value(v, visit);
                    }
                }
            }
        }
        if let Some(expected) = &e.expected {
            for v in expected.iter().flatten() {
                value(v, visit);
            }
        }
    }
    fn scan(steps: &[Step], visit: &mut impl FnMut(&Value) -> bool) {
        for step in steps {
            match step {
                Step::Assert(v) => value(v, visit),
                Step::Ordinary(s) => statements(std::slice::from_ref(s), visit),
                Step::Raises(_, body) => scan(body, visit),
                Step::If(v, a, b) => {
                    value(v, visit);
                    scan(a, visit);
                    scan(b, visit);
                }
                Step::Eval(e) | Step::BindEval(_, _, e) => evaluation(e, visit),
            }
        }
    }
    for test in tests {
        scan(&test.steps, visit);
    }
}
