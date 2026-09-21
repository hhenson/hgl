//! The accepted KEYED-ROUTE-TIMER scenario, through the real graph API.
use hgl_kernel::{Ctx, Graph, Node, NodeError, NodeResult, NodeSlot, RunConfig, run_simulation};
use hgl_nested::Children;
use hgl_store::{DictIn, DictOut, In, Out, OutputId, Store, Wake};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType, ScalarType};
use std::{cell::RefCell, rc::Rc};

type Log = Rc<RefCell<Vec<&'static str>>>;
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
fn slot(node: impl Node, label: &str, scheduled: bool) -> NodeSlot {
    NodeSlot {
        node: Box::new(node),
        label: label.into(),
        required: vec![],
        node_type: NodeType {
            uses_scheduler: true,
            schedule_on_start: scheduled,
            ..NodeType::default()
        },
    }
}
fn at(n: i64) -> EngineTime {
    EngineTime::from_micros(n + 1)
}

struct Feed {
    selectors: DictOut<i64>,
    prices: DictOut<i64>,
    step: usize,
}
impl Node for Feed {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let selectors = match self.step {
            0 | 7 => Some(0),
            2 => Some(1),
            _ => None,
        };
        if let Some(value) = selectors {
            let c = ctx.get_or_create(self.selectors, 0);
            ctx.set(c, value);
        }
        if self.step == 5 {
            ctx.remove(self.selectors, 0);
        }
        for (step, key, value) in [(0, 0, 2), (0, 1, 10), (1, 0, 3), (3, 0, 4), (6, 0, 5)] {
            if step == self.step {
                let c = ctx.get_or_create(self.prices, key);
                ctx.set(c, value);
            }
        }
        self.step += 1;
        if self.step < 11 {
            ctx.schedule_in(EngineDelta::STEP)?;
        }
        Ok(())
    }
}
struct Choose {
    selector: In<i64>,
    prices: DictIn<i64>,
    out: OutputId,
}
impl Node for Choose {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let price = ctx
            .store()
            .child(self.prices, ctx.get(self.selector))
            .ok_or_else(|| NodeError::new("missing selected price"))?;
        let r = ctx.store().bindings().input_reference(price.id());
        ctx.set_reference(self.out, r)
    }
}
struct Total {
    input: In<i64>,
    out: Out<i64>,
    sum: i64,
    log: Log,
}
impl Node for Total {
    fn start(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.log.borrow_mut().push("start");
        Ok(())
    }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if ctx.modified(self.input) {
            self.sum += ctx.get(self.input);
            ctx.set(self.out, self.sum);
            ctx.schedule_in(EngineDelta::from_micros(2))?;
        } else if ctx.is_scheduled_now() {
            ctx.set(self.out, -self.sum);
        }
        Ok(())
    }
    fn stop(&mut self, _: &mut Ctx<'_>) -> NodeResult {
        self.log.borrow_mut().push("stop");
        Ok(())
    }
}
struct Map {
    selectors: DictIn<i64>,
    prices: OutputId,
    out: DictOut<i64>,
    designation: Option<OutputId>,
    children: Children,
    log: Log,
}
impl Node for Map {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if let Some(reference) = self.designation.take() {
            ctx.set_reference(reference, ctx.store().reference(self.out.id()))?;
        }
        let removed: Vec<_> = ctx
            .store()
            .bindings()
            .removed_keys(self.selectors.id())
            .collect();
        for key in removed {
            self.children.remove(key, ctx)?;
            ctx.remove(self.out, key);
        }
        let added: Vec<_> = ctx
            .store()
            .bindings()
            .added_keys(self.selectors.id())
            .collect();
        for key in added {
            let source = ctx
                .store()
                .child(self.selectors, key)
                .ok_or_else(|| NodeError::new("missing selector"))?;
            let source = ctx.store().bindings().input_reference(source.id());
            let prices = self.prices;
            let log = Rc::clone(&self.log);
            let now = ctx.evaluation_time();
            let output = self.children.insert(key, ctx, |store| {
                let selector = store.add_input::<i64>(NodeId(0), true);
                let prices_in = store.add_dictionary_input::<i64>(NodeId(0), false);
                let ref_out = store.add_reference(NodeId(0), ScalarType::I64, false);
                let input = store.add_input::<i64>(NodeId(1), true);
                let out = store.add_output::<i64>(NodeId(1));
                let selected = store
                    .bindings()
                    .resolve(source)
                    .ok_or_else(|| NodeError::new("expired selector"))?;
                store
                    .bind(selector.id(), selected)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                store
                    .bind(prices_in.id(), prices)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                store
                    .follow(input.id(), ref_out, now, &mut Quiet)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                let choose = slot(
                    Choose {
                        selector,
                        prices: prices_in,
                        out: ref_out,
                    },
                    "choose",
                    true,
                );
                let mut total = slot(
                    Total {
                        input,
                        out,
                        sum: 0,
                        log,
                    },
                    "total",
                    false,
                );
                total.required.push(input.id());
                Ok((Graph::new("route".into(), vec![choose, total]), out))
            })?;
            ctx.attach(self.out, key, output)?;
        }
        self.children.evaluate(ctx)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}
fn map(
    store: &mut Store,
    owner: NodeId,
    selectors: OutputId,
    prices: OutputId,
    designation: Option<OutputId>,
    log: Log,
) -> Result<(Map, DictOut<i64>), Box<NodeError>> {
    let input = store.add_dictionary_input::<i64>(owner, true);
    let out = store.add_dictionary::<i64>(owner);
    store
        .bind(input.id(), selectors)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    Ok((
        Map {
            selectors: input,
            prices,
            out,
            designation,
            children: Children::new(),
            log,
        },
        out,
    ))
}
struct Outer {
    selectors: OutputId,
    prices: OutputId,
    reference: OutputId,
    children: Children,
    created: bool,
    log: Log,
}
impl Node for Outer {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if !self.created {
            let (selectors, prices, log) = (self.selectors, self.prices, Rc::clone(&self.log));
            let out = self.children.insert(0, ctx, |s| {
                let (node, out) = map(s, NodeId(0), selectors, prices, None, log)?;
                Ok((
                    Graph::new("outer".into(), vec![slot(node, "map", true)]),
                    out,
                ))
            })?;
            ctx.set_reference(self.reference, ctx.store().reference(out.id()))?;
            self.created = true;
        }
        self.children.evaluate(ctx)
    }
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.children.stop(ctx)
    }
}
#[derive(Debug, PartialEq)]
enum Tick {
    Idle,
    Value(i64),
    Remove,
}
struct Record {
    input: DictIn<i64>,
    ticks: Vec<Tick>,
}
impl Node for Record {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let tick = if ctx
            .store()
            .bindings()
            .removed_keys(self.input.id())
            .next()
            .is_some()
        {
            Tick::Remove
        } else if let Some(child) = ctx.store().child(self.input, 0) {
            if ctx.modified(child) {
                Tick::Value(ctx.get(child))
            } else {
                Tick::Idle
            }
        } else {
            Tick::Idle
        };
        self.ticks.push(tick);
        ctx.schedule_in(EngineDelta::STEP)
    }
}
fn run(outer: bool) -> Result<(), Box<NodeError>> {
    let mut store = Store::new();
    let log = Log::default();
    let selectors = store.add_dictionary::<i64>(NodeId(0));
    let prices = store.add_dictionary::<i64>(NodeId(0));
    let reference = store.add_reference(NodeId(1), ScalarType::I64, true);
    let owner = if outer {
        slot(
            Outer {
                selectors: selectors.id(),
                prices: prices.id(),
                reference,
                children: Children::new(),
                created: false,
                log: Rc::clone(&log),
            },
            "outer",
            true,
        )
    } else {
        let (node, _) = map(
            &mut store,
            NodeId(1),
            selectors.id(),
            prices.id(),
            Some(reference),
            Rc::clone(&log),
        )?;
        slot(node, "map", false)
    };
    let input = store.add_dictionary_input::<i64>(NodeId(2), false);
    store
        .follow(input.id(), reference, at(0), &mut Quiet)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let mut graph = Graph::new(
        "root".into(),
        vec![
            slot(
                Feed {
                    selectors,
                    prices,
                    step: 0,
                },
                "feed",
                true,
            ),
            owner,
            slot(
                Record {
                    input,
                    ticks: Vec::with_capacity(11),
                },
                "record",
                true,
            ),
        ],
    );
    run_simulation(
        &mut graph,
        &mut store,
        &RunConfig {
            start_time: at(0),
            end_time: at(11),
        },
    )
    .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let ticks = &graph
        .node::<Record>(NodeId(2))
        .ok_or_else(|| NodeError::new("missing recorder"))?
        .ticks;
    check_trace(ticks);
    assert_eq!(*log.borrow(), vec!["start", "stop", "start", "stop"]);
    Ok(())
}
fn check_trace(ticks: &Vec<Tick>) {
    assert_eq!(
        ticks,
        &vec![
            Tick::Value(2),
            Tick::Value(5),
            Tick::Value(15),
            Tick::Idle,
            Tick::Value(-15),
            Tick::Remove,
            Tick::Idle,
            Tick::Value(5),
            Tick::Idle,
            Tick::Value(-5),
            Tick::Idle
        ]
    );
}
#[test]
fn keyed_route_state_timer() -> Result<(), Box<NodeError>> {
    run(false)
}
#[test]
fn outer_graph_preserves_the_complete_keyed_route_trace() -> Result<(), Box<NodeError>> {
    run(true)
}
