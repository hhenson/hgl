static ALIAS_TRACE:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
pub fn alias_mark_i64(value:i64)->hgl_types::NodeResult<i64>{
 ALIAS_TRACE.fetch_update(std::sync::atomic::Ordering::SeqCst,std::sync::atomic::Ordering::SeqCst,|old|Some(old*10+value as usize)).unwrap();
 Ok(value)
}
pub fn alias_fail_i64(value:i64)->hgl_types::NodeResult<i64>{
 alias_mark_i64(value)?;
 Err(hgl_types::NodeError::coded("deliberate operand failure","yield.negative_duration"))
}
pub fn alias_finish_i64(expected:i64)->hgl_types::NodeResult{
 assert_eq!(ALIAS_TRACE.swap(0,std::sync::atomic::Ordering::SeqCst),expected as usize,"composition operands repeated or reordered");Ok(())
}
pub fn alias_verify_any(value:hgl_types::AnyValue)->hgl_types::NodeResult{
 assert_eq!(&*value.data(),&7_i64.to_le_bytes(),"failed composition replaced the prior owner");Ok(())
}
