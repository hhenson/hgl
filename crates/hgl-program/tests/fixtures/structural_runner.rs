mod native {
 pub use hgl_std_native::*;
 pub fn count_start_i64(_:i64)->hgl_types::NodeResult {crate::STARTS.fetch_add(1,std::sync::atomic::Ordering::SeqCst);Ok(())}
}
static STARTS:AtomicI64=AtomicI64::new(0);
mod evaluations;
use hgl_describe::{Registry,instantiate_complete};
use hgl_kernel::{run_simulation,RunConfig};
use hgl_store::Store;
use hgl_types::EngineTime;
use std::sync::atomic::{AtomicI64,Ordering};
mod values;
mod effects;
struct Provider;
static TRACE:AtomicI64=AtomicI64::new(0);
static FAIL:AtomicI64=AtomicI64::new(0);
impl effects::Native for Provider {
 fn mark_i64(value:i64)->hgl_types::NodeResult<i64> {
  TRACE.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|v|Some(v*10+value)).unwrap();
  if FAIL.load(Ordering::SeqCst)==value {return Err(hgl_types::NodeError::new("marker failure"));}
  Ok(value)
 }
}
fn main() {
 evaluations::main();
 let mut registry=Registry::new(); values::register(&mut registry).unwrap();
 let description=values::main(&registry).unwrap();
 for _ in 0..2 {
  let mut store=Store::new();store.provision_global_state();
  let mut built=instantiate_complete(&description,&registry,&mut store).unwrap();
  run_simulation(&mut built.graph,&mut store,&RunConfig{start_time:EngineTime::MIN_START,end_time:EngineTime::from_micros(10)}).unwrap();
  values::verify(&mut store);
 }
 // Stop after the second sparse update, while the nested map still exists.
 let mut store=Store::new();store.provision_global_state();
 let mut built=instantiate_complete(&description,&registry,&mut store).unwrap();
 run_simulation(&mut built.graph,&mut store,&RunConfig{start_time:EngineTime::MIN_START,end_time:EngineTime::from_micros(3)}).unwrap();
 let node=description.nodes.iter().position(|n|n.implementation.starts_with("structural_values::nested#")).unwrap();
 let map=built.outputs[node].unwrap();
 let tuple=store.bindings().child_output(map,7).unwrap();
 let list=store.bindings().fixed_output(tuple,0);
 let quote=store.bindings().fixed_output(list,1);
 let bid=store.scalar_output::<i64>(store.bindings().fixed_output(quote,0)).unwrap();
 let ask=store.scalar_output::<String>(store.bindings().fixed_output(quote,1)).unwrap();
 assert_eq!(store.output_value(bid),Some(1));
 assert_eq!(store.output_value(ask),Some("later".to_owned()));
 let mut registry=Registry::new();effects::register(&mut registry).unwrap();
 let description=effects::main(&registry).unwrap();
 for (end,fail,trace) in [(3,0,123),(10,0,1234),(10,2,12),(10,0,1234)] {
  TRACE.store(0,Ordering::SeqCst);FAIL.store(fail,Ordering::SeqCst);
  let mut store=Store::new();store.provision_global_state();
  let mut built=instantiate_complete(&description,&registry,&mut store).unwrap();
  let result=run_simulation(&mut built.graph,&mut store,&RunConfig{start_time:EngineTime::MIN_START,end_time:EngineTime::from_micros(end)});
  if fail!=0 {assert!(format!("{:?}",result.unwrap_err()).contains("marker failure"));}else{result.unwrap();}
  assert_eq!(TRACE.load(Ordering::SeqCst),trace);
  if end==10 && fail==0 {effects::verify(&mut store);}
 }
}
