//! Prepared factories own configuration only during graph construction.
use hgl_describe::{Builder, Registry, instantiate_complete};
use hgl_kernel::{Ctx, Node, NodeResult, RunConfig, run_simulation};
use hgl_store::{Out, Store};
use hgl_types::{EngineTime, NodeType, ScalarType, TsType};
use std::sync::Arc;
struct Prepared {
    value: String,
    output: Out<String>,
}
impl Node for Prepared {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.output, self.value.clone());
        Ok(())
    }
}
#[test]
fn factories_keep_owned_configuration_across_registry_clones_and_fresh_graphs() {
    let token = Arc::new(());
    let weak = Arc::downgrade(&token);
    let mut registry = Registry::new();
    let configuration = String::from("retained");
    registry
        .register_with(
            NodeType {
                name: "prepared",
                output: Some(TsType::Ts(ScalarType::Text)),
                schedule_on_start: true,
                ..NodeType::default()
            },
            move |ports| {
                assert!(Arc::strong_count(&token) >= 1);
                Ok(Prepared {
                    value: configuration.clone(),
                    output: ports.output()?,
                })
            },
        )
        .unwrap();
    let retained = registry.clone();
    drop(registry);
    assert!(weak.upgrade().is_some());
    for _ in 0..2 {
        let mut builder = Builder::new("fresh", &retained);
        builder.node("prepared", &[]).unwrap();
        let description = builder.finish().unwrap();
        let mut store = Store::new();
        let mut built = instantiate_complete(&description, &retained, &mut store).unwrap();
        run_simulation(
            &mut built.graph,
            &mut store,
            &RunConfig {
                start_time: EngineTime::MIN_START,
                end_time: EngineTime::MAX_END,
            },
        )
        .unwrap();
        assert_eq!(
            built
                .graph
                .node::<Prepared>(hgl_types::NodeId(0))
                .unwrap()
                .value,
            "retained"
        );
    }
    drop(retained);
    assert!(weak.upgrade().is_none());
}
