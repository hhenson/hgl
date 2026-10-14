//! A captured conditional pass abandons due-only work and retains future deadlines.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_kernel::{
    Ctx, Graph, Node, NodeError, NodeResult, NodeSlot,
    nested::{PreparedChild, PreparedSwitch},
};
use hgl_store::{Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
fn slot(node: impl Node, scheduled: bool) -> NodeSlot {
    NodeSlot {
        node: Box::new(node),
        label: "switch".into(),
        required: vec![],
        node_type: NodeType {
            uses_scheduler: true,
            schedule_on_start: scheduled,
            ..NodeType::default()
        },
    }
}
struct Writer {
    output: Out<i64>,
    count: i64,
    fail: bool,
}
impl Node for Writer {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(EngineDelta::from_micros(2))
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.count += 1;
        ctx.set(self.output, self.count);
        if self.fail && self.count == 1 {
            return Err(NodeError::coded("first pass", "value.missing_key"));
        }
        Ok(())
    }
}
struct Switch {
    children: PreparedSwitch<[Out<i64>; 2]>,
}
impl Node for Switch {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.evaluate(true, ctx, |_, _| Ok(()))?;
        Ok(())
    }
    fn discard_captured_pass(&mut self, ctx: &mut Ctx<'_>) {
        self.children.discard_captured_pass(ctx);
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}
struct Capture {
    child: PreparedChild<[Out<i64>; 2]>,
    started: bool,
    failures: usize,
}
impl Node for Capture {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if !self.started {
            self.child.start(ctx)?;
            self.started = true;
        }
        if let Some(error) = self.child.evaluate_captured(ctx)? {
            assert_eq!(error.code.as_deref(), Some("value.missing_key"));
            self.failures += 1;
        }
        ctx.schedule_children(self.child.next_scheduled_time());
        Ok(())
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.child.stop(ctx)
    }
}
#[test]
fn failed_selected_branch_does_not_retry_abandoned_due_work() -> NodeResult {
    let mut store = Store::new();
    let child = PreparedChild::new(&mut store, NodeId(0), |store| {
        let branch = PreparedChild::new(store, NodeId(0), |store| {
            let first = store.add_output::<i64>(NodeId(0));
            let second = store.add_output::<i64>(NodeId(1));
            Ok((
                Graph::new(
                    "branch".into(),
                    vec![
                        slot(
                            Writer {
                                output: first,
                                count: 0,
                                fail: true,
                            },
                            true,
                        ),
                        slot(
                            Writer {
                                output: second,
                                count: 0,
                                fail: false,
                            },
                            true,
                        ),
                    ],
                ),
                [first, second],
            ))
        })?;
        let outputs = *branch.boundary();
        Ok((
            Graph::new(
                "captured switch".into(),
                vec![slot(
                    Switch {
                        children: PreparedSwitch::new([vec![branch], vec![]]),
                    },
                    true,
                )],
            ),
            outputs,
        ))
    })?;
    let outputs = *child.boundary();
    let mut graph = Graph::new(
        "root".into(),
        vec![slot(
            Capture {
                child,
                started: false,
                failures: 0,
            },
            true,
        )],
    );
    graph.start(&mut store, EngineTime::MIN_START)?;
    graph.evaluate(&mut store, EngineTime::MIN_START)?;
    assert_eq!(store.output_value(outputs[0]), Some(1));
    assert_eq!(store.output_value(outputs[1]), None);
    let future = EngineTime::from_micros(EngineTime::MIN_START.micros() + 2);
    assert_eq!(graph.next_scheduled_time(), future);
    let (result, count) = count_in(|| graph.evaluate(&mut store, future));
    result?;
    assert_eq!(count, 0);
    assert_eq!(store.output_value(outputs[0]), Some(2));
    assert_eq!(store.output_value(outputs[1]), Some(1));
    assert_eq!(
        graph.node::<Capture>(NodeId(0)).map(|node| node.failures),
        Some(1)
    );
    assert_eq!(graph.next_scheduled_time(), EngineTime::FOREVER);
    graph.stop(&mut store, future)?;
    Ok(())
}
