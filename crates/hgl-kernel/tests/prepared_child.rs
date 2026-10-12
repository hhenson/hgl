//! Finite child activation, fresh state and next-cycle expiry allocate nothing.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_kernel::{Ctx, Graph, Node, NodeError, NodeResult, NodeSlot, nested::PreparedChild};
use hgl_store::{In, Out, OutputId, Store, Wake};
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
        label: "prepared".into(),
        required: vec![],
        node_type: NodeType {
            schedule_on_start: scheduled,
            uses_scheduler: true,
            ..NodeType::default()
        },
    }
}
struct Source(Out<i64>);
impl Node for Source {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.0, ctx.evaluation_time().micros());
        ctx.schedule_in(EngineDelta::STEP)
    }
}
#[derive(Clone, Copy, Debug)]
struct Boundary {
    input: In<i64>,
    output: Out<i64>,
}
struct Counter {
    boundary: Boundary,
    count: i64,
}
impl Node for Counter {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.count += 100;
        ctx.set(
            self.boundary.output,
            ctx.get(self.boundary.input) + self.count,
        );
        Ok(())
    }
}
struct Owner {
    source: Out<i64>,
    input: In<i64>,
    output: OutputId,
    children: Vec<PreparedChild<Boundary>>,
    selected: Option<usize>,
    held: OutputId,
}
impl Node for Owner {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let now = ctx.evaluation_time();
        let n = ctx.get(self.input);
        if matches!(n, 1 | 3 | 5) {
            if let Some(previous) = self.selected {
                self.children[previous].stop(ctx)?;
            }
            let selected =
                usize::try_from((n - 1) / 2).map_err(|error| NodeError::new(error.to_string()))?;
            let boundary = *self.children[selected].boundary();
            let source = self.source;
            self.children[selected].start_with(ctx, |store| {
                store
                    .sample(
                        boundary.input.id(),
                        store.reference(source.id()),
                        now,
                        &mut Quiet,
                    )
                    .map_err(|error| NodeError::new(format!("{error:?}")))
            })?;
            self.selected = Some(selected);
        }
        while ctx.take_child().is_some() {}
        let selected = self
            .selected
            .ok_or_else(|| NodeError::new("missing selection"))?;
        self.children[selected].evaluate(ctx)?;
        let target = ctx
            .store()
            .reference(self.children[selected].boundary().output.id());
        ctx.set_reference(self.output, target)?;
        if n == 1 {
            ctx.set_reference(self.held, target)?;
        }
        ctx.schedule_children(self.children[selected].next_scheduled_time());
        Ok(())
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if let Some(selected) = self.selected {
            self.children[selected].stop(ctx)?;
        }
        Ok(())
    }
}
struct Sink {
    input: In<i64>,
    held: In<i64>,
}
impl Node for Sink {
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
#[test]
fn finite_instances_start_fresh_and_preserve_retired_designations_only_for_stop_cycle()
-> Result<(), Box<NodeError>> {
    let mut store = Store::new();
    let source = store.add_output::<i64>(NodeId(0));
    let selector = store.add_input::<i64>(NodeId(1), true);
    store
        .bind(selector.id(), source.id())
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    let output = store.add_reference(NodeId(1), ScalarType::I64, false);
    let held = store.add_reference(NodeId(1), ScalarType::I64, false);
    let current_input = store.add_input::<i64>(NodeId(2), true);
    let held_input = store.add_input::<i64>(NodeId(2), true);
    store
        .follow(current_input.id(), output, EngineTime::NEVER, &mut Quiet)
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    store
        .follow(held_input.id(), held, EngineTime::NEVER, &mut Quiet)
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    let children = prepare_instances(&mut store)?;
    store.reserve_reference_target(source.id(), 3);
    store.prepare_collection_inputs();
    let mut graph = Graph::new(
        "root".into(),
        vec![
            slot(Source(source), true),
            slot(
                Owner {
                    source,
                    input: selector,
                    output,
                    children,
                    selected: None,
                    held,
                },
                false,
            ),
            slot(
                Sink {
                    input: current_input,
                    held: held_input,
                },
                false,
            ),
        ],
    );
    graph.start(&mut store, EngineTime::MIN_START)?;
    let counts = store.bindings().storage_counts();
    for (n, expected, old) in [
        (1, 101, Some(101)),
        (2, 202, Some(202)),
        (3, 103, Some(202)),
        (4, 204, None),
        (5, 105, None),
        (6, 206, None),
    ] {
        let (result, allocations) =
            count_in(|| graph.evaluate(&mut store, EngineTime::from_micros(n)));
        result?;
        assert_eq!(allocations, 0, "activation, stop and expiry at cycle {n}");
        let sink = graph
            .node::<Sink>(NodeId(2))
            .ok_or_else(|| NodeError::new("sink"))?;
        assert_eq!(store.get(sink.input), expected);
        assert_eq!(store.input_valid(sink.held.id()), old.is_some());
        if let Some(old) = old {
            assert_eq!(store.get(sink.held), old);
        }
        assert_eq!(store.bindings().storage_counts()[..3], counts[..3]);
    }
    let (result, allocations) = count_in(|| graph.stop(&mut store, EngineTime::from_micros(6)));
    result?;
    assert_eq!(allocations, 0);
    Ok(())
}

fn prepare_instances(store: &mut Store) -> Result<Vec<PreparedChild<Boundary>>, Box<NodeError>> {
    let mut children = Vec::new();
    for _ in 0..3 {
        let child = PreparedChild::new(store, NodeId(1), |store| {
            let boundary = Boundary {
                input: store.add_input::<i64>(NodeId(0), true),
                output: store.add_output::<i64>(NodeId(0)),
            };
            store.reserve_reference_target(boundary.output.id(), 2);
            Ok((
                Graph::new(
                    "fresh child".into(),
                    vec![slot(Counter { boundary, count: 0 }, false)],
                ),
                boundary,
            ))
        })?;
        children.push(child);
    }
    Ok(children)
}

#[derive(Clone, Debug, Default)]
struct Hooks {
    starts: std::rc::Rc<std::cell::Cell<u32>>,
    stops: std::rc::Rc<std::cell::Cell<u32>>,
}
struct LifecycleProbe {
    hooks: Hooks,
    fail: bool,
}
impl Node for LifecycleProbe {
    fn start(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.hooks.starts.set(self.hooks.starts.get() + 1);
        if self.fail {
            return Err(NodeError::new("start failed"));
        }
        Ok(())
    }
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
    fn stop(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.hooks.stops.set(self.hooks.stops.get() + 1);
        Ok(())
    }
}
struct FailureOwner {
    failing: PreparedChild<()>,
    unbound: PreparedChild<()>,
    unused: PreparedChild<()>,
    foreign: PreparedChild<()>,
}
impl Node for FailureOwner {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        assert!(self.unused.evaluate(ctx).is_err());
        assert!(self.foreign.start(ctx).is_err());
        assert!(self.foreign.stop(ctx).is_err());
        assert!(
            self.unbound
                .start_with(ctx, |_| Err(NodeError::new("capture failed")))
                .is_err()
        );
        self.unbound.stop(ctx)?;
        assert!(self.failing.start(ctx).is_err());
        self.failing.stop(ctx)?;
        self.failing.stop(ctx)?;
        self.unused.stop(ctx)?;
        Ok(())
    }
}
fn lifecycle_child(
    store: &mut Store,
    owner: NodeId,
    hooks: Hooks,
    fail: bool,
) -> Result<PreparedChild<()>, Box<NodeError>> {
    PreparedChild::new(store, owner, |_| {
        Ok((
            Graph::new(
                "lifecycle child".into(),
                vec![slot(LifecycleProbe { hooks, fail }, false)],
            ),
            (),
        ))
    })
}
#[test]
fn prepared_failure_cleanup_does_not_start_unused_children_or_stop_a_failed_start_node()
-> Result<(), Box<NodeError>> {
    let mut store = Store::new();
    let started = Hooks::default();
    let failed = Hooks::default();
    let unused = Hooks::default();
    let unbound = Hooks::default();
    let foreign = Hooks::default();
    let first = started.clone();
    let second = failed.clone();
    let failing = PreparedChild::new(&mut store, NodeId(0), |_| {
        Ok((
            Graph::new(
                "failing child".into(),
                vec![
                    slot(
                        LifecycleProbe {
                            hooks: first,
                            fail: false,
                        },
                        false,
                    ),
                    slot(
                        LifecycleProbe {
                            hooks: second,
                            fail: true,
                        },
                        false,
                    ),
                ],
            ),
            (),
        ))
    })?;
    let owner = FailureOwner {
        failing,
        unbound: lifecycle_child(&mut store, NodeId(0), unbound.clone(), false)?,
        unused: lifecycle_child(&mut store, NodeId(0), unused.clone(), false)?,
        foreign: lifecycle_child(&mut store, NodeId(1), foreign.clone(), false)?,
    };
    let mut graph = Graph::new(
        "failure root".into(),
        vec![
            slot(owner, true),
            slot(
                LifecycleProbe {
                    hooks: Hooks::default(),
                    fail: false,
                },
                false,
            ),
        ],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    graph.evaluate(&mut store, EngineTime::MIN_START)?;
    graph.stop(&mut store, EngineTime::MIN_START)?;
    assert_eq!((started.starts.get(), started.stops.get()), (1, 1));
    assert_eq!((failed.starts.get(), failed.stops.get()), (1, 0));
    for hooks in [unused, unbound, foreign] {
        assert_eq!((hooks.starts.get(), hooks.stops.get()), (0, 0));
    }
    Ok(())
}

struct UnusedNested<T> {
    children: Vec<PreparedChild<T>>,
}
impl<T: 'static> Node for UnusedNested<T> {
    fn eval(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        for child in &mut self.children {
            child.stop(ctx)?;
        }
        Ok(())
    }
}
struct DiscardPrepared(PreparedChild<Vec<hgl_store::ScopeId>>);
impl Node for DiscardPrepared {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.0.stop(ctx)?;
        self.0.stop(ctx)?;
        Ok(())
    }
}
#[test]
fn unused_outer_retires_all_unstarted_descendants_without_hooks_or_allocations()
-> Result<(), Box<NodeError>> {
    let mut store = Store::new();
    let hooks = Hooks::default();
    let descendant_hooks = hooks.clone();
    let outer = PreparedChild::new(&mut store, NodeId(0), |store| {
        let middle = PreparedChild::new(store, NodeId(0), |store| {
            let leaf = lifecycle_child(store, NodeId(0), descendant_hooks, false)?;
            let scopes = vec![leaf.scope()];
            Ok((
                Graph::new(
                    "unused middle".into(),
                    vec![slot(
                        UnusedNested {
                            children: vec![leaf],
                        },
                        false,
                    )],
                ),
                scopes,
            ))
        })?;
        let mut scopes = middle.boundary().clone();
        scopes.push(middle.scope());
        Ok((
            Graph::new(
                "unused outer".into(),
                vec![slot(
                    UnusedNested {
                        children: vec![middle],
                    },
                    false,
                )],
            ),
            scopes,
        ))
    })?;
    let scopes = outer.boundary().clone();
    assert!(
        scopes
            .iter()
            .all(|&s| store.bindings().child_owner(s).is_some())
    );
    let mut graph = Graph::new(
        "discard root".into(),
        vec![slot(DiscardPrepared(outer), true)],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    let (result, allocations) = count_in(|| graph.evaluate(&mut store, EngineTime::MIN_START));
    result?;
    assert_eq!(allocations, 0);
    assert!(
        scopes
            .iter()
            .all(|&s| store.bindings().child_owner(s).is_none())
    );
    assert_eq!((hooks.starts.get(), hooks.stops.get()), (0, 0));
    graph.stop(&mut store, EngineTime::MIN_START)?;
    Ok(())
}

struct FiniteSwitch {
    branches: hgl_kernel::nested::PreparedSwitch<()>,
    step: usize,
}
impl Node for FiniteSwitch {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let enabled = self.step != 1;
        self.step += 1;
        self.branches.evaluate(enabled, ctx, |_, ()| Ok(()))?;
        ctx.schedule_in(EngineDelta::STEP)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.branches.stop(ctx)
    }
}
#[test]
fn exhausted_finite_switch_never_restarts_a_stopped_lifetime() -> Result<(), Box<NodeError>> {
    let mut store = Store::new();
    let yes = Hooks::default();
    let no = Hooks::default();
    let branches = hgl_kernel::nested::PreparedSwitch::new([
        vec![lifecycle_child(&mut store, NodeId(0), yes.clone(), false)?],
        vec![lifecycle_child(&mut store, NodeId(0), no.clone(), false)?],
    ]);
    let mut graph = Graph::new(
        "bounded switch".into(),
        vec![slot(FiniteSwitch { branches, step: 0 }, true)],
    );
    store.prepare_collection_inputs();
    graph.start(&mut store, EngineTime::MIN_START)?;
    for step in 0..2 {
        let now = EngineTime::from_micros(EngineTime::MIN_START.micros() + step);
        let (result, allocations) = count_in(|| graph.evaluate(&mut store, now));
        result?;
        assert_eq!(allocations, 0);
    }
    let now = EngineTime::from_micros(EngineTime::MIN_START.micros() + 2);
    let error = graph.evaluate(&mut store, now).unwrap_err();
    assert!(error.message.contains("activation bound exhausted"));
    let (stopped, allocations) = count_in(|| graph.stop(&mut store, now));
    stopped?;
    assert_eq!(allocations, 0);
    for hooks in [yes, no] {
        assert_eq!((hooks.starts.get(), hooks.stops.get()), (1, 1));
    }
    Ok(())
}
