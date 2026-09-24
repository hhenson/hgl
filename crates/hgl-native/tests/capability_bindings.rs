//! Executable contract for call-scoped service forwarding to native helpers.
/// Generated shared native contract used by this conformance fixture.
#[path = "support/capability_interface.rs"]
pub mod interface;

use interface::{Logger, Native};

struct Provider;
impl Native for Provider {
    fn bit_and(lhs: i64, rhs: i64) -> i64 {
        lhs & rhs
    }

    fn audit(value: i64, logger: &mut dyn Logger) -> i64 {
        logger.info("native helper");
        value
    }
}

#[derive(Default)]
struct RecordingLogger(Vec<String>);
impl Logger for RecordingLogger {
    fn info(&mut self, message: &str) {
        self.0.push(message.to_owned());
    }
}

fn middle(value: i64, logger: &mut dyn Logger) -> i64 {
    Provider::audit(value, logger)
}

#[test]
fn helper_borrows_the_callers_logger_and_preserves_duplicate_values() {
    let mut logger = RecordingLogger::default();
    let inputs = [Some(2), None, Some(2), Some(3)];
    let outputs = inputs.map(|tick| tick.map(|value| middle(value, &mut logger)));
    assert_eq!(outputs, inputs);
    assert_eq!(
        logger.0,
        ["native helper", "native helper", "native helper"]
    );
    assert_eq!(Provider::bit_and(6, 3), 2);
}
