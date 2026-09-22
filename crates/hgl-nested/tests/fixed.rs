//! Four accepted fixed-collection switch/map traces through real child graphs.
use hgl_kernel::{Ctx, Graph, Node, NodeError, NodeResult, NodeSlot, RunConfig, run_simulation};
use hgl_nested::Children;
use hgl_store::{DictIn, DictOut, In, Kind, Out, OutputId, Reference, Store, Wake};
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
    prices: [Out<i64>; 2],
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
                let c = self.prices[usize::try_from(key).unwrap_or_else(|_| unreachable!())];
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
    sources: [Reference; 2],
    out: OutputId,
}
impl Node for Choose {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let n =
            usize::try_from(ctx.get(self.selector)).map_err(|e| NodeError::new(e.to_string()))?;
        ctx.set_reference(self.out, self.sources[n])
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
    sources: [Reference; 2],
    shape: Kind,
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
            let sources = self.sources;
            let shape = self.shape.clone();
            let log = Rc::clone(&self.log);
            let now = ctx.evaluation_time();
            let output = self.children.insert(key, ctx, |store| {
                let selector = store.add_input::<i64>(NodeId(0), true);
                let ref_out =
                    store.add_shaped_output(NodeId(0), Kind::Reference(Box::new(shape.clone())));
                let view = store.add_shaped_input(NodeId(1), shape, true);
                let child = store.bindings().fixed_input(view, 0);
                let leaf = store.bindings().fixed_input(child, 0);
                let input = store
                    .scalar_input::<i64>(leaf)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                let out = store.add_output::<i64>(NodeId(1));
                let selected = store
                    .bindings()
                    .resolve(source)
                    .ok_or_else(|| NodeError::new("expired selector"))?;
                store
                    .bind(selector.id(), selected)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                store
                    .follow(view, ref_out, now, &mut Quiet)
                    .map_err(|e| NodeError::new(format!("{e:?}")))?;
                let choose = slot(
                    Choose {
                        selector,
                        sources,
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
    sources: [Reference; 2],
    shape: Kind,
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
            sources,
            shape,
            out,
            designation: None,
            children: Children::new(),
            log,
        },
        out,
    ))
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
#[expect(
    clippy::too_many_lines,
    reason = "complete child graph wiring and independent trace comparison are kept together"
)]
fn run(bundle: bool, peered: bool) -> Result<(), Box<NodeError>> {
    let mut store = Store::new();
    let log = Log::default();
    let selectors = store.add_dictionary::<i64>(NodeId(0));
    let pair = |bundle: bool, child: Kind| {
        if bundle {
            Kind::Bundle(vec![
                ("left".into(), child.clone()),
                ("right".into(), child),
            ])
        } else {
            Kind::List(Box::new(child), 2)
        }
    };
    let shape = pair(bundle, pair(!bundle, Kind::Ts(ScalarType::I64)));
    let a = store.add_shaped_output(NodeId(0), shape.clone());
    let b = store.add_shaped_output(NodeId(0), shape.clone());
    let prices = [a, b].map(|o| {
        let child = store.bindings().fixed_output(o, 0);
        store
            .scalar_output::<i64>(store.bindings().fixed_output(child, 0))
            .unwrap_or_else(|_| unreachable!())
    });
    let sources = if peered {
        [store.reference(a), store.reference(b)]
    } else {
        [items(&mut store, a)?, items(&mut store, b)?]
    };
    let reference = store.add_reference(NodeId(1), ScalarType::I64, true);
    let (node, out) = map(
        &mut store,
        NodeId(1),
        selectors.id(),
        sources,
        shape,
        Rc::clone(&log),
    )?;
    store
        .set_reference(reference, store.reference(out.id()), at(0), &mut Quiet)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let owner = slot(node, "map", false);
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
    let case = format!(
        "{}_{}_graph_{}",
        if bundle { "tsb" } else { "tsl" },
        if bundle { "tsl" } else { "tsb" },
        if peered { "owned" } else { "assembled" }
    );
    for line in include_str!("../../hgl-store/tests/fixed_support/accepted.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields[0] != case {
            continue;
        }
        let actual = if fields[1] == "/events" {
            format!(
                "[{}]",
                log.borrow()
                    .iter()
                    .map(|s| format!("{s:?}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        } else if fields[2] == "length" {
            ticks.len().to_string()
        } else {
            let n: usize = fields[1]
                .trim_start_matches("/ticks/")
                .split('/')
                .next()
                .unwrap_or_else(|| unreachable!())
                .parse()
                .map_err(|e| NodeError::new(format!("{e}")))?;
            match ticks[n] {
                Tick::Idle => "null".into(),
                Tick::Value(v) => v.to_string(),
                Tick::Remove => "true".into(),
            }
        };
        assert_eq!(actual, fields[3], "{case} {}", fields[1]);
    }
    Ok(())
}
fn items(store: &mut Store, output: OutputId) -> Result<Reference, Box<NodeError>> {
    let kind = store.bindings().output(output).kind.clone();
    if !kind.fixed() {
        return Ok(store.reference(output));
    }
    let children = (0..kind.len())
        .map(|n| items(store, store.bindings().fixed_output(output, n)))
        .collect::<Result<Vec<_>, _>>()?;
    store
        .items_reference(kind, children)
        .map_err(|e| NodeError::new(format!("{e:?}")))
}
#[test]
fn accepted_fixed_child_graphs() -> Result<(), Box<NodeError>> {
    for bundle in [false, true] {
        for peered in [false, true] {
            run(bundle, peered)?;
        }
    }
    Ok(())
}
