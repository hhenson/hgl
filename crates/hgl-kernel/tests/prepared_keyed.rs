//! Fresh keyed lifetimes, surviving state, deadline routing and uncaught errors.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_kernel::{
    Ctx, Graph, Node, NodeError, NodeResult, NodeSlot, nested::PreparedChild,
    prepared_keyed::PreparedKeyed,
};
use hgl_store::{In, Out, Store, Wake};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn slot(node: impl Node, scheduled: bool) -> NodeSlot {
    NodeSlot {
        node: Box::new(node),
        label: "keyed".into(),
        required: vec![],
        node_type: NodeType {
            uses_scheduler: true,
            schedule_on_start: scheduled,
            ..NodeType::default()
        },
    }
}

struct Counter {
    output: Out<i64>,
    count: i64,
    fail: bool,
}
impl Node for Counter {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if self.fail {
            return Err(NodeError::new("uncaught keyed failure"));
        }
        self.count += 1;
        ctx.set(self.output, self.count);
        ctx.schedule_in(EngineDelta::STEP)
    }
}

struct Owner {
    children: PreparedKeyed<Out<i64>>,
    step: usize,
    recreate: bool,
}
impl Node for Owner {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        match self.step {
            0 => {
                self.children.insert(11, ctx, |_, _| Ok(()))?;
                self.children.insert(22, ctx, |_, _| Ok(()))?;
            }
            1 => self.children.insert(11, ctx, |_, _| {
                Err(NodeError::new("existing lifetime rebound"))
            })?,
            2 if self.recreate => self.children.remove(11, ctx)?,
            3 if self.recreate => self.children.insert(11, ctx, |_, _| Ok(()))?,
            _ => {}
        }
        self.step += 1;
        self.children.evaluate(ctx)?;
        ctx.schedule_in(EngineDelta::STEP)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}

type Pool = (Vec<PreparedChild<Out<i64>>>, Vec<Out<i64>>);
fn pool(store: &mut Store, count: usize, fail: bool) -> NodeResult<Pool> {
    let mut children = Vec::new();
    let mut outputs = Vec::new();
    for _ in 0..count {
        let child = PreparedChild::new(store, NodeId(0), |store| {
            let output = store.add_output::<i64>(NodeId(0));
            Ok((
                Graph::new(
                    "keyed member".into(),
                    vec![slot(
                        Counter {
                            output,
                            count: 0,
                            fail,
                        },
                        true,
                    )],
                ),
                output,
            ))
        })?;
        outputs.push(*child.boundary());
        children.push(child);
    }
    Ok((children, outputs))
}

#[test]
fn prepared_members_recreate_fresh_and_removed_deadlines_never_wake_old_scopes() -> NodeResult {
    let mut store = Store::new();
    let (first, outputs) = pool(&mut store, 2, false)?;
    let (second, sibling) = pool(&mut store, 1, false)?;
    let children = PreparedKeyed::new(vec![(11, first), (22, second)]);
    let mut graph = Graph::new(
        "root".into(),
        vec![slot(
            Owner {
                children,
                step: 0,
                recreate: true,
            },
            true,
        )],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    for (step, old, fresh, surviving) in [
        (0, Some(1), None, 1),
        (1, Some(2), None, 2),
        (2, Some(2), None, 3),
        (3, None, Some(1), 4),
        (4, None, Some(2), 5),
    ] {
        let now = EngineTime::from_micros(EngineTime::MIN_START.micros() + step);
        let (result, allocations) = count_in(|| graph.evaluate(&mut store, now));
        result?;
        assert_eq!(allocations, 0, "complete keyed evaluation at step {step}");
        assert_eq!(store.output_value(outputs[0]), old);
        assert_eq!(store.output_value(outputs[1]), fresh);
        assert_eq!(store.output_value(sibling[0]), Some(surviving));
    }
    let (result, allocations) = count_in(|| {
        graph.stop(
            &mut store,
            EngineTime::from_micros(EngineTime::MIN_START.micros() + 4),
        )
    });
    result?;
    assert_eq!(allocations, 0);
    Ok(())
}

#[test]
fn uncaught_member_failure_escapes_keyed_owner() -> NodeResult {
    let mut store = Store::new();
    let (first, _) = pool(&mut store, 1, true)?;
    let (second, _) = pool(&mut store, 1, false)?;
    let children = PreparedKeyed::new(vec![(11, first), (22, second)]);
    let mut graph = Graph::new(
        "root".into(),
        vec![slot(
            Owner {
                children,
                step: 0,
                recreate: false,
            },
            true,
        )],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    let error = graph
        .evaluate(&mut store, EngineTime::MIN_START)
        .unwrap_err();
    assert!(error.message.contains("uncaught keyed failure"));
    graph.stop(&mut store, EngineTime::MIN_START)?;
    Ok(())
}

#[test]
fn exhausted_pool_does_not_restart_stopped_member() -> NodeResult {
    let mut store = Store::new();
    let (first, _) = pool(&mut store, 1, false)?;
    let (second, _) = pool(&mut store, 1, false)?;
    let children = PreparedKeyed::new(vec![(11, first), (22, second)]);
    let mut graph = Graph::new(
        "root".into(),
        vec![slot(
            Owner {
                children,
                step: 0,
                recreate: true,
            },
            true,
        )],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    for step in 0..3 {
        graph.evaluate(
            &mut store,
            EngineTime::from_micros(EngineTime::MIN_START.micros() + step),
        )?;
    }
    let now = EngineTime::from_micros(EngineTime::MIN_START.micros() + 3);
    let error = graph.evaluate(&mut store, now).unwrap_err();
    assert!(error.message.contains("activation bound exhausted"));
    graph.stop(&mut store, now)?;
    Ok(())
}

#[derive(Clone, Default)]
struct Hooks {
    starts: std::rc::Rc<std::cell::Cell<usize>>,
    stops: std::rc::Rc<std::cell::Cell<usize>>,
}
struct LifecycleProbe {
    hooks: Hooks,
    fail_start: bool,
    fail_stop: bool,
}
impl Node for LifecycleProbe {
    fn start(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.hooks.starts.set(self.hooks.starts.get() + 1);
        if self.fail_start {
            return Err(NodeError::new("keyed start failure"));
        }
        Ok(())
    }
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
    fn stop(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.hooks.stops.set(self.hooks.stops.get() + 1);
        if self.fail_stop {
            return Err(NodeError::new("keyed stop failure"));
        }
        Ok(())
    }
}
struct LifecycleOwner {
    children: PreparedKeyed<()>,
    started: bool,
}
impl Node for LifecycleOwner {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if self.started {
            return self.children.remove(11, ctx);
        }
        self.children.insert(11, ctx, |_, ()| Ok(()))?;
        self.started = true;
        self.children.evaluate(ctx)?;
        ctx.schedule_in(EngineDelta::STEP)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}

#[test]
fn creation_start_and_membership_stop_failures_escape_with_ordinary_cleanup() -> NodeResult {
    for fail_start in [true, false] {
        let mut store = Store::new();
        let hooks = Hooks::default();
        let observed = hooks.clone();
        let child = PreparedChild::new(&mut store, NodeId(0), |_| {
            Ok((
                Graph::new(
                    "lifecycle member".into(),
                    vec![slot(
                        LifecycleProbe {
                            hooks,
                            fail_start,
                            fail_stop: !fail_start,
                        },
                        false,
                    )],
                ),
                (),
            ))
        })?;
        let scope = child.scope();
        let children = PreparedKeyed::new(vec![(11, vec![child])]);
        let mut graph = Graph::new(
            "lifecycle root".into(),
            vec![slot(
                LifecycleOwner {
                    children,
                    started: false,
                },
                true,
            )],
        );
        store.prepare_collection_inputs();
        graph.start(&mut store, EngineTime::MIN_START)?;
        let mut now = EngineTime::MIN_START;
        let first = graph.evaluate(&mut store, now);
        let error = if fail_start {
            first.unwrap_err()
        } else {
            first?;
            now = EngineTime::from_micros(now.micros() + 1);
            graph.evaluate(&mut store, now).unwrap_err()
        };
        assert!(error.message.contains(if fail_start {
            "keyed start failure"
        } else {
            "keyed stop failure"
        }));
        assert_eq!(
            error
                .error_value()
                .context
                .as_ref()
                .map(|context| context.phase.as_str()),
            Some(if fail_start { "start" } else { "stop" })
        );
        graph.stop(&mut store, now)?;
        assert!(store.bindings().child_owner(scope).is_none());
        assert_eq!(observed.starts.get(), 1);
        assert_eq!(observed.stops.get(), usize::from(!fail_start));
    }
    Ok(())
}

struct FutureMember {
    output: Out<i64>,
    count: i64,
    fail_first: bool,
}
impl Node for FutureMember {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(EngineDelta::from_micros(2))
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.count += 1;
        ctx.set(self.output, self.count);
        if self.fail_first && self.count == 1 {
            return Err(NodeError::coded(
                "first keyed pass failed",
                "value.missing_key",
            ));
        }
        Ok(())
    }
}
struct CapturedKeys {
    children: PreparedKeyed<Out<i64>>,
    initialized: bool,
}
impl Node for CapturedKeys {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if !self.initialized {
            self.children.insert(11, ctx, |_, _| Ok(()))?;
            self.children.insert(22, ctx, |_, _| Ok(()))?;
            self.initialized = true;
        }
        self.children.evaluate(ctx)
    }
    fn discard_captured_pass(&mut self, ctx: &mut Ctx<'_>) {
        self.children.discard_captured_pass(ctx);
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}
struct CaptureKeys {
    child: PreparedChild<[Out<i64>; 2]>,
    started: bool,
    failures: usize,
}
impl Node for CaptureKeys {
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
fn enclosing_capture_discards_sibling_due_pass_and_preserves_both_future_wakes() -> NodeResult {
    let mut store = Store::new();
    let child = PreparedChild::new(&mut store, NodeId(0), |store| {
        let mut pools = Vec::new();
        let mut outputs = Vec::new();
        for key in [11, 22] {
            let member = PreparedChild::new(store, NodeId(0), |store| {
                let output = store.add_output::<i64>(NodeId(0));
                Ok((
                    Graph::new(
                        "future member".into(),
                        vec![slot(
                            FutureMember {
                                output,
                                count: 0,
                                fail_first: key == 11,
                            },
                            true,
                        )],
                    ),
                    output,
                ))
            })?;
            outputs.push(*member.boundary());
            pools.push((key, vec![member]));
        }
        let children = PreparedKeyed::new(pools);
        Ok((
            Graph::new(
                "captured keyed graph".into(),
                vec![slot(
                    CapturedKeys {
                        children,
                        initialized: false,
                    },
                    true,
                )],
            ),
            [outputs[0], outputs[1]],
        ))
    })?;
    let outputs = *child.boundary();
    let mut graph = Graph::new(
        "capture root".into(),
        vec![slot(
            CaptureKeys {
                child,
                started: false,
                failures: 0,
            },
            true,
        )],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    graph.evaluate(&mut store, EngineTime::MIN_START)?;
    assert_eq!(store.output_value(outputs[0]), Some(1));
    assert_eq!(store.output_value(outputs[1]), None);
    let future = EngineTime::from_micros(EngineTime::MIN_START.micros() + 2);
    assert_eq!(graph.next_scheduled_time(), future);
    let (result, allocations) = count_in(|| graph.evaluate(&mut store, future));
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(store.output_value(outputs[0]), Some(2));
    assert_eq!(store.output_value(outputs[1]), Some(1));
    assert_eq!(
        graph
            .node::<CaptureKeys>(NodeId(0))
            .map(|owner| owner.failures),
        Some(1)
    );
    assert_eq!(graph.next_scheduled_time(), EngineTime::FOREVER);
    graph.stop(&mut store, future)?;
    Ok(())
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
    children: PreparedKeyed<RetargetBoundary>,
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
            self.children.insert(11, ctx, |store, boundary| {
                sample(store, boundary.member, first, now)?;
                sample(store, boundary.shared, shared, now)
            })?;
        } else if self.step == 1 {
            let second = self.second;
            self.children.refresh(11, ctx, |store, boundary| {
                sample(store, boundary.member, second, now)
            })?;
        }
        self.step += 1;
        self.children.evaluate(ctx)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}

#[test]
fn live_member_retarget_preserves_state_and_does_not_resample_shared_consumers() -> NodeResult {
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
    let children = PreparedKeyed::new(vec![(11, vec![child])]);
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
                    children,
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

type Events = std::rc::Rc<std::cell::RefCell<Vec<(bool, i64, u8)>>>;
struct OrderProbe {
    events: Events,
    key: i64,
    generation: u8,
}
impl Node for OrderProbe {
    fn start(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.events
            .borrow_mut()
            .push((true, self.key, self.generation));
        Ok(())
    }
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
    fn stop(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.events
            .borrow_mut()
            .push((false, self.key, self.generation));
        Ok(())
    }
}
const REMOVED: &[i64] = &[113, 3, 250, 64, 1, 197, 0, 255, 127];
struct ManyKeys {
    children: PreparedKeyed<()>,
    step: usize,
}
impl Node for ManyKeys {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        match self.step {
            0 => {
                for key in 0..256 {
                    self.children.insert(key, ctx, |_, ()| Ok(()))?;
                }
            }
            1 => {
                for &key in REMOVED {
                    self.children.remove(key, ctx)?;
                }
            }
            2 => {
                for &key in REMOVED.iter().rev() {
                    self.children.insert(key, ctx, |_, ()| Ok(()))?;
                }
            }
            _ => unreachable!("finite many-key lifecycle"),
        }
        self.step += 1;
        self.children.evaluate(ctx)?;
        if self.step < 3 {
            ctx.schedule_in(EngineDelta::STEP)
        } else {
            Ok(())
        }
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}

#[test]
fn many_keys_remove_out_of_creation_order_and_recreated_keys_cleanup_in_reverse_order() -> NodeResult
{
    let mut store = Store::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::with_capacity(1024)));
    let mut pools = Vec::new();
    for key in 0..256 {
        let mut pool = Vec::new();
        for generation in 0..2 {
            let events = std::rc::Rc::clone(&events);
            pool.push(PreparedChild::new(&mut store, NodeId(0), |_| {
                Ok((
                    Graph::new(
                        "ordered member".into(),
                        vec![slot(
                            OrderProbe {
                                events,
                                key,
                                generation,
                            },
                            false,
                        )],
                    ),
                    (),
                ))
            })?);
        }
        pools.push((key, pool));
    }
    let children = PreparedKeyed::new(pools);
    let mut graph = Graph::new(
        "many-key root".into(),
        vec![slot(ManyKeys { children, step: 0 }, true)],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    for step in 0..3 {
        let now = EngineTime::from_micros(EngineTime::MIN_START.micros() + step);
        let (result, allocations) = count_in(|| graph.evaluate(&mut store, now));
        result?;
        assert_eq!(allocations, 0);
    }
    let (result, allocations) = count_in(|| {
        graph.stop(
            &mut store,
            EngineTime::from_micros(EngineTime::MIN_START.micros() + 2),
        )
    });
    result?;
    assert_eq!(allocations, 0);
    let events = events.borrow();
    let starts = events
        .iter()
        .filter(|(start, _, _)| *start)
        .map(|(_, key, generation)| (*key, *generation))
        .collect::<Vec<_>>();
    let expected_starts = (0..256)
        .map(|key| (key, 0))
        .chain(REMOVED.iter().rev().map(|&key| (key, 1)))
        .collect::<Vec<_>>();
    assert_eq!(starts, expected_starts);
    let stops = events
        .iter()
        .filter(|(start, _, _)| !*start)
        .map(|(_, key, generation)| (*key, *generation))
        .collect::<Vec<_>>();
    let expected_stops = REMOVED
        .iter()
        .map(|&key| (key, 0))
        .chain(REMOVED.iter().map(|&key| (key, 1)))
        .chain(
            (0..256)
                .rev()
                .filter(|key| !REMOVED.contains(key))
                .map(|key| (key, 0)),
        )
        .collect::<Vec<_>>();
    assert_eq!(stops, expected_stops);
    Ok(())
}
