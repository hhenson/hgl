//! Live capture boundary refresh preserves child state and exact shared notifications.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_kernel::{Ctx, Graph, Node, NodeError, NodeResult, NodeSlot, nested::PreparedChild};
use hgl_store::{In, Out, Store, Wake};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
fn slot(node: impl Node, scheduled: bool) -> NodeSlot {
    NodeSlot {
        node: Box::new(node),
        label: "refresh".into(),
        required: vec![],
        node_type: NodeType {
            uses_scheduler: true,
            schedule_on_start: scheduled,
            ..NodeType::default()
        },
    }
}
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
#[derive(Clone, Copy, Debug)]
struct RetargetBoundary {
    member: In<i64>,
    shared: In<i64>,
    result: Out<i64>,
    shared_ticks: Out<i64>,
}
struct MemberCounter {
    boundary: RetargetBoundary,
    count: i64,
}
impl Node for MemberCounter {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.count += 1;
        ctx.set(
            self.boundary.result,
            self.count * 100 + ctx.get(self.boundary.member) + ctx.get(self.boundary.shared),
        );
        Ok(())
    }
}
struct SharedCounter {
    boundary: RetargetBoundary,
    count: i64,
}
impl Node for SharedCounter {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.count += 1;
        ctx.set(self.boundary.shared_ticks, self.count);
        Ok(())
    }
}
struct RetargetSource {
    first: Out<i64>,
    second: Out<i64>,
    shared: Out<i64>,
    control: Out<i64>,
    step: i64,
}
impl Node for RetargetSource {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        match self.step {
            0 => {
                ctx.set(self.first, 5);
                ctx.set(self.shared, 10);
            }
            1 => ctx.set(self.second, 7),
            2 => ctx.set(self.second, 9),
            _ => unreachable!("finite retarget source"),
        }
        ctx.set(self.control, self.step);
        self.step += 1;
        if self.step < 3 {
            ctx.schedule_in(EngineDelta::STEP)
        } else {
            Ok(())
        }
    }
}
struct RetargetOwner {
    child: PreparedChild<RetargetBoundary>,
    first: Out<i64>,
    second: Out<i64>,
    shared: Out<i64>,
    step: usize,
}
fn sample(store: &mut Store, input: In<i64>, output: Out<i64>, now: EngineTime) -> NodeResult {
    store.unbind(input.id());
    store
        .sample(input.id(), store.reference(output.id()), now, &mut Quiet)
        .map_err(|error| NodeError::new(format!("{error:?}")))
}
impl Node for RetargetOwner {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let now = ctx.evaluation_time();
        if self.step == 0 {
            let (first, shared) = (self.first, self.shared);
            self.child.start_bound(ctx, |store, boundary| {
                sample(store, boundary.member, first, now)?;
                sample(store, boundary.shared, shared, now)
            })?;
        } else if self.step == 1 {
            let second = self.second;
            self.child.refresh_bound(ctx, |store, boundary| {
                sample(store, boundary.member, second, now)
            })?;
        }
        self.step += 1;
        self.child.evaluate(ctx)?;
        ctx.schedule_children(self.child.next_scheduled_time());
        Ok(())
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.child.stop(ctx)
    }
}

#[test]
fn refresh_bound_keeps_notifications_in_parent_scope_and_shared_consumers_silent() -> NodeResult {
    let mut store = Store::new();
    let first = store.add_output::<i64>(NodeId(0));
    let second = store.add_output::<i64>(NodeId(0));
    let shared = store.add_output::<i64>(NodeId(0));
    let control = store.add_output::<i64>(NodeId(0));
    let input = store.add_input::<i64>(NodeId(1), true);
    store
        .bind(input.id(), control.id())
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    let child = retarget_child(&mut store, shared)?;
    let boundary = *child.boundary();
    for output in [first, second, shared] {
        store.reserve_reference_target(output.id(), 3);
    }

    let mut graph = Graph::new(
        "retarget root".into(),
        vec![
            slot(
                RetargetSource {
                    first,
                    second,
                    shared,
                    control,
                    step: 0,
                },
                true,
            ),
            slot(
                RetargetOwner {
                    child,
                    first,
                    second,
                    shared,
                    step: 0,
                },
                false,
            ),
        ],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    for (step, expected) in [115, 217, 319].into_iter().enumerate() {
        let now = EngineTime::from_micros(
            EngineTime::MIN_START.micros()
                + i64::try_from(step).map_err(|error| NodeError::new(error.to_string()))?,
        );
        let (result, allocations) = count_in(|| graph.evaluate(&mut store, now));
        result?;
        assert_eq!(allocations, 0);
        assert_eq!(store.output_value(boundary.result), Some(expected));
        assert_eq!(store.output_value(boundary.shared_ticks), Some(1));
    }
    graph.stop(
        &mut store,
        EngineTime::from_micros(EngineTime::MIN_START.micros() + 2),
    )?;
    Ok(())
}

fn retarget_child(
    store: &mut Store,
    shared: Out<i64>,
) -> NodeResult<PreparedChild<RetargetBoundary>> {
    PreparedChild::new(store, NodeId(1), |store| {
        let boundary = RetargetBoundary {
            member: store.add_input::<i64>(NodeId(0), true),
            shared: store.add_input::<i64>(NodeId(0), true),
            result: store.add_output::<i64>(NodeId(0)),
            shared_ticks: store.add_output::<i64>(NodeId(1)),
        };
        let watched = store.add_input::<i64>(NodeId(1), true);
        // Both consumers share the source; only the member consumer changes peer.
        let counter = RetargetBoundary {
            shared: watched,
            ..boundary
        };
        store
            .bind(watched.id(), shared.id())
            .map_err(|error| NodeError::new(format!("{error:?}")))?;
        Ok((
            Graph::new(
                "retargeted member".into(),
                vec![
                    slot(MemberCounter { boundary, count: 0 }, false),
                    slot(
                        SharedCounter {
                            boundary: counter,
                            count: 0,
                        },
                        false,
                    ),
                ],
            ),
            boundary,
        ))
    })
}
