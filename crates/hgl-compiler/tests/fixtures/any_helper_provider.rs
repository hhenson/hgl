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

    static GENERATOR_TRACE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    pub fn generator_begin_i64(_: i64) -> hgl_types::NodeResult {
        assert_eq!(GENERATOR_TRACE.load(std::sync::atomic::Ordering::SeqCst),0,"generator evaluated before start");
        Ok(())
    }
    pub fn generator_mark_i64(value:i64)->hgl_types::NodeResult<i64> {
        GENERATOR_TRACE.fetch_update(std::sync::atomic::Ordering::SeqCst,std::sync::atomic::Ordering::SeqCst,|old|Some(old*10+value.unsigned_abs() as usize)).unwrap();
        Ok(value)
    }
    pub fn generator_time_i64(value:i64)->hgl_types::NodeResult<hgl_types::EngineDelta> {
        generator_mark_i64(value)?;
        Ok(hgl_types::EngineDelta::from_micros(if value<0 {-1}else{1}))
    }
    pub fn generator_finish_i64(expected:i64)->hgl_types::NodeResult {
        assert_eq!(GENERATOR_TRACE.swap(0,std::sync::atomic::Ordering::SeqCst),expected as usize,"generator repeated or reordered operands");
        Ok(())
    }

    pub fn generator_reject_time_i64(value:i64)->hgl_types::NodeResult<hgl_types::EngineDelta> {
        generator_mark_i64(value)?;
        Err(hgl_types::NodeError::coded("deliberate time operand failure","yield.negative_duration"))
    }
