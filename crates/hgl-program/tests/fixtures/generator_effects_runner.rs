use hgl_stdlib::std_native as native;
use hgl_describe::{Registry, instantiate_complete};
use hgl_kernel::{RunConfig, run_simulation};
use hgl_store::Store;
use hgl_types::EngineTime;
use std::sync::atomic::{AtomicI64, Ordering};
struct Provider;
static TRACE: AtomicI64=AtomicI64::new(0);
static FAIL: AtomicI64=AtomicI64::new(0);
fn mark(value:i64)->Result<i64,Box<hgl_types::NodeError>> {
    TRACE.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|trace|Some(trace*10+value)).unwrap();
    let failure=FAIL.load(Ordering::SeqCst);
    if failure==value || failure==TRACE.load(Ordering::SeqCst) { return Err(hgl_types::NodeError::new("marker failure")); }
    Ok(value)
}
fn run(registry: &Registry, description: &hgl_describe::GraphDescription, fail:i64, end:i64, trace:i64, count:i64, last:Option<i64>, error:Option<&str>, held:Option<i64>) {
    TRACE.store(0,Ordering::SeqCst);
    FAIL.store(fail,Ordering::SeqCst);
    let mut store=Store::new();
    store.global_state().provision();
    let mut built=instantiate_complete(description,registry,&mut store).unwrap();
    let result=run_simulation(&mut built.graph,&mut store,&RunConfig {start_time:EngineTime::MIN_START,end_time:EngineTime::from_micros(end)});
    match error {
        Some(expected)=>assert!(format!("{:?}",result.unwrap_err()).contains(expected)),
        None=>{result.unwrap();},
    }
    assert_eq!(TRACE.load(Ordering::SeqCst),trace);
    let output=built.outputs[0].unwrap();
    assert_eq!(store.output_value_erased(output),held.map(hgl_types::ScalarValue::I64));
    let key=store.global_state().bind::<i64>("count").unwrap();
    assert_eq!(store.global_state().get(key).unwrap(),count);
    if let Some(value)=last {
        let key=store.global_state().bind::<i64>("last").unwrap();
        assert_eq!(store.global_state().get(key).unwrap(),value);
    }
}
