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
    static PREFLIGHT_CALLS: AtomicUsize = AtomicUsize::new(0);
    pub fn preflight_later_i64(_: i64) -> hgl_types::NodeResult<i64> {
        panic!("later operand executed before failed retention");
    }
    pub fn preflight_first_i64(value: i64) -> hgl_types::NodeResult<i64> {
        assert_eq!(PREFLIGHT_CALLS.fetch_add(1, Ordering::SeqCst), 0);
        Ok(value)
    }
    pub fn preflight_finish_bool(unchanged: bool) -> hgl_types::NodeResult {
        assert!(unchanged, "failed constructor modified existing destination");
        assert_eq!(PREFLIGHT_CALLS.swap(0, Ordering::SeqCst), 1);
        Ok(())
    }

}
