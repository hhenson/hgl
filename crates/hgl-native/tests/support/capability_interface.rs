// Generated from checked HGL native declarations; do not edit.
/// Implementation of the shared native value interface.
pub trait Native {
    /// `checks.native_provider::bit_and`, with value-level arguments and result.
    fn r#bit_and(r#lhs: i64, r#rhs: i64) -> i64;
    /// `checks.native_provider::audit`, with value-level arguments and result.
    fn r#audit(r#value: i64, hgl_cap_logger: &mut dyn Logger) -> i64;
}
/// Call-scoped logging supplied by the caller.
pub trait Logger {
    /// Emit an informational message in the caller context.
    fn info(&mut self, message: &str);
}
