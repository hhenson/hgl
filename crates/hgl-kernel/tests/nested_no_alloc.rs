//! Steady TSD/REF/nested ticks and timer replacement allocate nothing.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_kernel::nested::Children;
use hgl_kernel::{Ctx, Graph, Node, NodeError, NodeResult, NodeSlot};
use hgl_store::{DictIn, DictOut, In, Out, OutputId, Store, Wake};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType, ScalarType};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
fn slot(node: impl Node, scheduled: bool) -> NodeSlot {
    NodeSlot {
        node: Box::new(node),
        label: "node".into(),
        required: vec![],
        node_type: NodeType {
            uses_scheduler: true,
            schedule_on_start: scheduled,
            ..NodeType::default()
        },
    }
}
struct Source {
    positive: Out<i64>,
    negative: Out<i64>,
    reference: OutputId,
}
impl Node for Source {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let n = ctx.evaluation_time().micros();
        ctx.set(self.positive, n);
        ctx.set(self.negative, -n);
        let target = if n % 2 == 0 {
            self.negative
        } else {
            self.positive
        };
        ctx.set_reference(self.reference, ctx.store().reference(target.id()))?;
        ctx.schedule_in(EngineDelta::STEP)
    }
}
struct Double {
    input: In<i64>,
    out: Out<i64>,
}
impl Node for Double {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.input) * 2);
        ctx.schedule_in(EngineDelta::from_micros(2))
    }
}
struct Owner {
    children: Children,
    reference: OutputId,
    out: DictOut<i64>,
    created: bool,
}
impl Node for Owner {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if !self.created {
            let reference = self.reference;
            let now = ctx.evaluation_time();
            let out = self.children.insert(0, ctx, |store| {
                let input = store.add_input::<i64>(NodeId(0), true);
                let out = store.add_output::<i64>(NodeId(0));
                store
                    .follow(input.id(), reference, now, &mut Quiet)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                Ok((
                    Graph::new("child".into(), vec![slot(Double { input, out }, false)]),
                    out,
                ))
            })?;
            ctx.attach(self.out, 0, out)?;
            self.created = true;
        }
        self.children.evaluate(ctx)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}
struct Sink {
    input: DictIn<i64>,
    count: i64,
    sum: i64,
}
impl Node for Sink {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let child = ctx
            .store()
            .child(self.input, 0)
            .ok_or_else(|| NodeError::new("missing output"))?;
        self.count += 1;
        self.sum += ctx.get(child);
        Ok(())
    }
}
#[test]
fn repeated_reference_rebinding_and_timer_replacement_are_allocation_free()
-> Result<(), Box<NodeError>> {
    let mut store = Store::new();
    let positive = store.add_output::<i64>(NodeId(0));
    let negative = store.add_output::<i64>(NodeId(0));
    let reference = store.add_reference(NodeId(0), ScalarType::I64, false);
    let out = store.add_dictionary::<i64>(NodeId(1));
    let input = store.add_dictionary_input::<i64>(NodeId(2), true);
    store
        .bind(input.id(), out.id())
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let mut graph = Graph::new(
        "root".into(),
        vec![
            slot(
                Source {
                    positive,
                    negative,
                    reference,
                },
                true,
            ),
            slot(
                Owner {
                    children: Children::new(),
                    reference,
                    out,
                    created: false,
                },
                true,
            ),
            slot(
                Sink {
                    input,
                    count: 0,
                    sum: 0,
                },
                false,
            ),
        ],
    );
    graph.start(&mut store, EngineTime::MIN_START)?;
    for n in 1..=10 {
        graph.evaluate(&mut store, EngineTime::from_micros(n))?;
    }
    let (result, allocations) = count_in(|| -> NodeResult {
        for n in 11..=10_010 {
            graph.evaluate(&mut store, EngineTime::from_micros(n))?;
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    let sink = graph
        .node::<Sink>(NodeId(2))
        .ok_or_else(|| NodeError::new("missing sink"))?;
    assert_eq!((sink.count, sink.sum), (10_010, -10_010));
    graph.stop(&mut store, EngineTime::from_micros(10_010))?;
    Ok(())
}
