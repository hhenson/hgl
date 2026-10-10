use std::sync::atomic::{AtomicI64,Ordering};
static TRACE:AtomicI64=AtomicI64::new(0);
struct Provider;
fn mark(value:i64)->hgl_types::NodeResult<i64> {
    TRACE.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|trace|Some(trace*10+value)).unwrap();
    if value==9 {Err(hgl_types::NodeError::new("marker failure"))} else {Ok(value)}
}
fn run(registry:hgl_describe::Registry,description:hgl_describe::GraphDescription,trace:i64,after:Option<i64>) {
    TRACE.store(0,Ordering::SeqCst);
    let mut store=hgl_store::Store::new();store.global_state().provision();
    let mut built=hgl_describe::instantiate_complete(&description,&registry,&mut store).unwrap();
    let result=hgl_kernel::run_simulation(&mut built.graph,&mut store,&hgl_kernel::RunConfig {start_time:hgl_types::EngineTime::MIN_START,end_time:hgl_types::EngineTime::MAX_END});
    if after.is_some() {result.unwrap();} else {assert!(format!("{:?}",result.unwrap_err()).contains("marker failure"));}
    assert_eq!(TRACE.load(Ordering::SeqCst),trace);
    let before=store.global_state().bind::<i64>("before").unwrap();assert_eq!(store.global_state().get(before).unwrap(),1);
    let handle=store.global_state().bind::<i64>("after").unwrap();
    if let Some(after)=after {assert_eq!(store.global_state().get(handle).unwrap(),after);} else {assert!(store.global_state().get(handle).is_err());}
}
