//! Accepted timer, cancellation, sibling and failure cases.
use hgl_kernel::{Ctx, Graph, Node, NodeError, NodeResult, NodeSlot, RunConfig, run_simulation};
use hgl_nested::Children;
use hgl_store::{DictIn, DictOut, In, Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};
use std::{cell::RefCell, rc::Rc};
type Delta = Vec<(i64, Option<i64>)>;
type Log = Rc<RefCell<Vec<&'static str>>>;
fn at(t: i64) -> EngineTime {
    EngineTime::from_micros(t + 1)
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
struct Feed {
    out: DictOut<i64>,
    ticks: Vec<Delta>,
    index: usize,
}
impl Node for Feed {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        for &(key, value) in &self.ticks[self.index] {
            if let Some(value) = value {
                let child = ctx.get_or_create(self.out, key);
                ctx.set(child, value);
            } else {
                ctx.remove(self.out, key);
            }
        }
        self.index += 1;
        if self.index < self.ticks.len() {
            ctx.schedule_in(EngineDelta::STEP)?;
        }
        Ok(())
    }
}
struct Timer {
    input: In<i64>,
    out: Out<i64>,
    log: Log,
    failure: u8,
}
impl Node for Timer {
    fn start(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.log.borrow_mut().push("start");
        if self.failure == 1 {
            return Err(NodeError::new("start failure"));
        }
        Ok(())
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if self.failure == 2 {
            self.log.borrow_mut().push(if ctx.get(self.input) < 0 {
                "eval(-1)"
            } else {
                "eval(1)"
            });
            if ctx.get(self.input) < 0 {
                return Err(NodeError::new("eval failure"));
            }
        }
        if ctx.modified(self.input) {
            ctx.set(self.out, ctx.get(self.input));
            ctx.schedule_in(EngineDelta::from_micros(ctx.get(self.input)))?;
        } else if ctx.is_scheduled_now() {
            ctx.set(self.out, -1);
        }
        Ok(())
    }
    fn stop(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.log.borrow_mut().push("stop");
        Ok(())
    }
}
struct Mapped {
    input: DictIn<i64>,
    out: DictOut<i64>,
    children: Children,
    log: Log,
    failure: u8,
}
impl Node for Mapped {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let removed: Vec<_> = ctx
            .store()
            .bindings()
            .removed_keys(self.input.id())
            .collect();
        for key in removed {
            self.children.remove(key, ctx)?;
            ctx.remove(self.out, key);
        }
        let added: Vec<_> = ctx.store().bindings().added_keys(self.input.id()).collect();
        for key in added {
            let child = ctx
                .store()
                .child(self.input, key)
                .ok_or_else(|| NodeError::new("missing input"))?;
            let reference = ctx.store().bindings().input_reference(child.id());
            let log = Rc::clone(&self.log);
            let failure = self.failure;
            let output = self.children.insert(key, ctx, |s| {
                let input = s.add_input::<i64>(NodeId(0), true);
                let out = s.add_output::<i64>(NodeId(0));
                let source = s
                    .bindings()
                    .resolve(reference)
                    .ok_or_else(|| NodeError::new("expired input"))?;
                s.bind(input.id(), source)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                if failure == 3 {
                    return Err(NodeError::new("build failure"));
                }
                let mut node = slot(
                    Timer {
                        input,
                        out,
                        log,
                        failure,
                    },
                    true,
                );
                node.required.push(input.id());
                Ok((Graph::new("timer".into(), vec![node]), out))
            })?;
            ctx.attach(self.out, key, output)?;
        }
        self.children.evaluate(ctx)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}
struct Record {
    input: DictIn<i64>,
    ticks: Vec<Delta>,
}
impl Node for Record {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let mut delta: Delta = ctx
            .store()
            .bindings()
            .removed_keys(self.input.id())
            .map(|key| (key, None))
            .collect();
        for &key in ctx.store().bindings().changed_keys(self.input.id()) {
            if let Some(child) = ctx.store().child(self.input, key)
                && ctx.modified(child)
            {
                delta.push((key, Some(ctx.get(child))));
            }
        }
        delta.sort_unstable();
        delta.dedup();
        self.ticks.push(delta);
        ctx.schedule_in(EngineDelta::STEP)
    }
}
fn run(ticks: Vec<Delta>, failure: u8) -> Result<(Graph, Store, Log, bool), Box<NodeError>> {
    let count = i64::try_from(ticks.len()).map_err(|e| NodeError::new(e.to_string()))?;
    let mut store = Store::new();
    let log = Log::default();
    let feed = store.add_dictionary::<i64>(NodeId(0));
    let input = store.add_dictionary_input::<i64>(NodeId(1), true);
    let out = store.add_dictionary::<i64>(NodeId(1));
    let record = store.add_dictionary_input::<i64>(NodeId(2), false);
    store
        .bind(input.id(), feed.id())
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    store
        .bind(record.id(), out.id())
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let mut graph = Graph::new(
        "root".into(),
        vec![
            slot(
                Feed {
                    out: feed,
                    ticks,
                    index: 0,
                },
                true,
            ),
            slot(
                Mapped {
                    input,
                    out,
                    children: Children::new(),
                    log: Rc::clone(&log),
                    failure,
                },
                false,
            ),
            slot(
                Record {
                    input: record,
                    ticks: Vec::new(),
                },
                true,
            ),
        ],
    );
    let result = run_simulation(
        &mut graph,
        &mut store,
        &RunConfig {
            start_time: at(0),
            end_time: at(count),
        },
    );
    if failure == 0 {
        assert!(result.is_ok(), "{result:?}");
    } else {
        assert!(result.is_err(), "failure must escape its owner");
    }
    Ok((graph, store, log, result.is_err()))
}
fn check(ticks: Vec<Delta>, expected: &[Delta]) -> Result<(), Box<NodeError>> {
    let (graph, _, _, _) = run(ticks, 0)?;
    assert_eq!(
        graph
            .node::<Record>(NodeId(2))
            .ok_or_else(|| NodeError::new("record"))?
            .ticks,
        expected
    );
    Ok(())
}
#[test]
fn child_only_deadlines() -> Result<(), Box<NodeError>> {
    check(
        vec![
            vec![(0, Some(2))],
            vec![],
            vec![],
            vec![(0, None)],
            vec![],
            vec![(0, Some(3))],
            vec![],
            vec![],
            vec![],
            vec![],
        ],
        &[
            vec![(0, Some(2))],
            vec![],
            vec![(0, Some(-1))],
            vec![(0, None)],
            vec![],
            vec![(0, Some(3))],
            vec![],
            vec![],
            vec![(0, Some(-1))],
            vec![],
        ],
    )
}
#[test]
fn removal_cancels_the_old_deadline() -> Result<(), Box<NodeError>> {
    check(
        vec![
            vec![(0, Some(4))],
            vec![],
            vec![(0, None)],
            vec![],
            vec![],
            vec![(0, Some(2))],
            vec![],
            vec![],
            vec![],
        ],
        &[
            vec![(0, Some(4))],
            vec![],
            vec![(0, None)],
            vec![],
            vec![],
            vec![(0, Some(2))],
            vec![],
            vec![(0, Some(-1))],
            vec![],
        ],
    )
}
#[test]
fn sibling_timers_and_input_on_a_due_deadline() -> Result<(), Box<NodeError>> {
    check(
        vec![
            vec![(0, Some(3)), (1, Some(2))],
            vec![],
            vec![(1, Some(4))],
            vec![],
            vec![(0, None)],
            vec![(0, Some(2))],
            vec![],
            vec![],
            vec![(0, None), (1, None)],
            vec![],
        ],
        &[
            vec![(0, Some(3)), (1, Some(2))],
            vec![],
            vec![(1, Some(4))],
            vec![(0, Some(-1))],
            vec![(0, None)],
            vec![(0, Some(2))],
            vec![(1, Some(-1))],
            vec![(0, Some(-1))],
            vec![(0, None), (1, None)],
            vec![],
        ],
    )
}
#[test]
fn evaluation_failure_stops_the_started_child_once() -> Result<(), Box<NodeError>> {
    let (_, _, log, _) = run(vec![vec![(0, Some(1))], vec![(0, Some(-1))], vec![]], 2)?;
    assert_eq!(*log.borrow(), vec!["start", "eval(1)", "eval(-1)", "stop"]);
    Ok(())
}
#[test]
fn start_failure_does_not_stop_the_failing_child() -> Result<(), Box<NodeError>> {
    let (_, mut store, log, _) = run(vec![vec![(0, Some(1))], vec![]], 1)?;
    assert_eq!(*log.borrow(), vec!["start"]);
    store.begin_cycle(at(10));
    assert_eq!(store.bindings().storage_counts()[3], 3);
    Ok(())
}
#[test]
fn construction_failure_detaches_partial_ports() -> Result<(), Box<NodeError>> {
    let (_, mut store, log, _) = run(vec![vec![(0, Some(1))], vec![]], 3)?;
    assert!(log.borrow().is_empty());
    store.begin_cycle(at(10));
    assert_eq!(store.bindings().storage_counts()[3], 3);
    Ok(())
}

#[test]
fn long_key_churn_keeps_endpoint_scope_and_subscription_storage_bounded()
-> Result<(), Box<NodeError>> {
    let campaign = |cycles: usize| -> Result<_, Box<NodeError>> {
        let ticks = (0..cycles)
            .map(|n| vec![(0, if n % 2 == 0 { Some(4) } else { None })])
            .collect();
        let (_, mut store, log, _) = run(ticks, 0)?;
        store.begin_cycle(at(i64::try_from(cycles)
            .map_err(|e| NodeError::new(e.to_string()))?
            + 1));
        assert_eq!(log.borrow().len(), cycles);
        Ok(store.bindings().storage_counts())
    };
    assert_eq!(campaign(1000)?, campaign(10)?);
    Ok(())
}
