struct Provider;
mod native {
    pub use hgl_stdlib::std_native::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static STARTED: AtomicUsize = AtomicUsize::new(0);
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    static TRACE: AtomicUsize = AtomicUsize::new(0);
    pub fn owner_begin_i64(_: i64) -> hgl_types::NodeResult {
        assert_eq!(STARTED.swap(1, Ordering::SeqCst), 0);
        CALLS.store(0, Ordering::SeqCst);
        TRACE.store(0, Ordering::SeqCst);
        Ok(())
    }
    pub fn owner_mark_i64(value: i64) -> hgl_types::NodeResult<i64> {
        assert_eq!(STARTED.load(Ordering::SeqCst), 1, "constructor evaluated before start");
        CALLS.fetch_add(1, Ordering::SeqCst);
        TRACE.store(TRACE.load(Ordering::SeqCst) * 10 + value as usize, Ordering::SeqCst);
        Ok(value)
    }
    pub fn owner_finish_i64(_: i64) -> hgl_types::NodeResult {
        assert_eq!(CALLS.load(Ordering::SeqCst), 4, "constructor operand repeated");
        assert_eq!(TRACE.load(Ordering::SeqCst), 2121, "constructor argument order");
        STARTED.store(0, Ordering::SeqCst);
        Ok(())
    }

static STAGE_TRACE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn stage_mark_i64(value:i64)->hgl_types::NodeResult<i64> {
    use std::sync::atomic::Ordering;
    STAGE_TRACE.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|old|Some(old*10+value as usize)).unwrap();
    Ok(value)
}
pub fn stage_fail_i64(value:i64)->hgl_types::NodeResult<i64> {
    stage_mark_i64(value)?;
    Err(hgl_types::NodeError::coded("deliberate operand failure","yield.negative_duration"))
}
pub fn stage_finish_i64(expected:i64)->hgl_types::NodeResult {
    assert_eq!(STAGE_TRACE.swap(0,std::sync::atomic::Ordering::SeqCst),expected as usize,"operand order/once/failure barrier");
    Ok(())
}

}
