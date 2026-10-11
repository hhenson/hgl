use std::sync::atomic::{AtomicUsize, Ordering};
static TEXT_TRACE: AtomicUsize = AtomicUsize::new(0);
pub fn text_mark_i64(value: i64) -> hgl_types::NodeResult<i64> {
    TEXT_TRACE.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
        Some(old * 10 + value as usize)
    }).unwrap();
    Ok(value)
}
pub fn text_fail_i64(value: i64) -> hgl_types::NodeResult<i64> {
    text_mark_i64(value)?;
    Err(hgl_types::NodeError::coded("deliberate operand failure", "yield.negative_duration"))
}
pub fn text_finish_i64(expected: i64) -> hgl_types::NodeResult {
    assert_eq!(TEXT_TRACE.swap(0, Ordering::SeqCst), expected as usize,
        "computed text reordered, repeated, or escaped a failing operand");
    Ok(())
}
