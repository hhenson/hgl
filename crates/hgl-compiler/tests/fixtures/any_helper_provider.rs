use std::sync::atomic::{AtomicUsize, Ordering};
static HELPER_CALLS: AtomicUsize = AtomicUsize::new(0);
static HELPER_TRACE: AtomicUsize = AtomicUsize::new(0);
pub fn helper_begin_i64(_: i64) -> hgl_types::NodeResult {
    HELPER_CALLS.store(0, Ordering::SeqCst);
    HELPER_TRACE.store(0, Ordering::SeqCst);
    Ok(())
}
pub fn helper_mark_i64(value: i64) -> hgl_types::NodeResult<i64> {
    HELPER_CALLS.fetch_add(1, Ordering::SeqCst);
    HELPER_TRACE.store(HELPER_TRACE.load(Ordering::SeqCst) * 10 + value as usize, Ordering::SeqCst);
    Ok(value)
}
pub fn helper_finish_i64(_: i64) -> hgl_types::NodeResult {
    assert_eq!(HELPER_CALLS.load(Ordering::SeqCst), 6, "helper argument count");
    assert_eq!(HELPER_TRACE.load(Ordering::SeqCst), 212121, "helper argument order");
    Ok(())
}
