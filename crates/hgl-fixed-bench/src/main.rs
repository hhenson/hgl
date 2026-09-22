//! Fixed collection benchmark; paired with bench/baselines/cpp/fixed.cpp.
#![expect(clippy::print_stdout, reason = "benchmark command line output")]
use hgl_kernel::{Ctx, Graph, Node, NodeError, NodeResult, NodeSlot, RunConfig, run_simulation};
use hgl_store::{BindError, In, Kind, Out, OutputId, Reference, Store, Wake};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType, ScalarType};
use std::time::Instant;
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
fn slot(node: impl Node, scheduled: bool) -> NodeSlot {
    NodeSlot {
        node: Box::new(node),
        label: "benchmark".into(),
        required: vec![],
        node_type: NodeType {
            uses_scheduler: true,
            schedule_on_start: scheduled,
            ..NodeType::default()
        },
    }
}
struct Pulse {
    out: Out<i64>,
    count: i64,
    limit: i64,
}
impl Node for Pulse {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, self.count);
        self.count += 1;
        if self.count < self.limit {
            ctx.schedule_in(EngineDelta::STEP)?;
        }
        Ok(())
    }
}
struct Produce {
    input: In<i64>,
    leaves: Vec<Out<i64>>,
    offset: i64,
}
impl Node for Produce {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let value = ctx.get(self.input) + self.offset;
        for (n, &out) in self.leaves.iter().enumerate() {
            ctx.set(
                out,
                value + i64::try_from(n).unwrap_or_else(|_| unreachable!()),
            );
        }
        Ok(())
    }
}
struct Route {
    input: In<i64>,
    out: OutputId,
    sources: [Reference; 2],
}
impl Node for Route {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set_reference(
            self.out,
            self.sources[usize::from(ctx.get(self.input) % 2 != 0)],
        )
    }
}
struct Sink {
    inputs: Vec<In<i64>>,
    sum: i64,
    count: i64,
}
impl Node for Sink {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        for &input in &self.inputs {
            self.sum += ctx.get(input);
        }
        self.count += 1;
        Ok(())
    }
}
fn shape() -> Kind {
    Kind::List(
        Box::new(Kind::Bundle(vec![
            ("left".into(), Kind::Scalar(ScalarType::I64)),
            ("right".into(), Kind::Scalar(ScalarType::I64)),
        ])),
        2,
    )
}
fn trigger(store: &mut Store, owner: NodeId, source: Out<i64>) -> Result<In<i64>, BindError> {
    let input = store.add_input(owner, true);
    store.bind(input.id(), source.id())?;
    Ok(input)
}
fn producer(
    store: &mut Store,
    nodes: &mut Vec<NodeSlot>,
    pulse: Out<i64>,
    offset: i64,
) -> Result<OutputId, BindError> {
    let owner = NodeId(u32::try_from(nodes.len()).unwrap_or_else(|_| unreachable!()));
    let input = trigger(store, owner, pulse)?;
    let output = store.add_shaped_output(owner, shape());
    let mut leaves = Vec::new();
    for a in 0..2 {
        for b in 0..2 {
            let child = store.bindings().fixed_output(output, a);
            leaves.push(store.scalar_output(store.bindings().fixed_output(child, b))?);
        }
    }
    nodes.push(slot(
        Produce {
            input,
            leaves,
            offset,
        },
        false,
    ));
    Ok(output)
}
fn assembled(
    store: &mut Store,
    nodes: &mut Vec<NodeSlot>,
    pulse: Out<i64>,
) -> Result<Reference, BindError> {
    let mut leaves = Vec::new();
    for offset in 0..4 {
        let owner = NodeId(u32::try_from(nodes.len()).unwrap_or_else(|_| unreachable!()));
        let input = trigger(store, owner, pulse)?;
        let out = store.add_output::<i64>(owner);
        leaves.push(store.reference(out.id()));
        nodes.push(slot(
            Produce {
                input,
                leaves: vec![out],
                offset,
            },
            false,
        ));
    }
    let mut children = Vec::new();
    for pair in leaves.chunks(2) {
        children.push(store.items_reference(shape().child(0).clone(), pair.to_vec())?);
    }
    store.items_reference(shape(), children)
}
fn graph(mode: &str, cycles: i64) -> Result<(Store, Graph, NodeId), BindError> {
    let mut store = Store::new();
    let pulse = store.add_output(NodeId(0));
    let mut nodes = vec![slot(
        Pulse {
            out: pulse,
            count: 0,
            limit: cycles,
        },
        true,
    )];
    let source = if mode == "assembled" {
        assembled(&mut store, &mut nodes, pulse)?
    } else {
        let output = producer(&mut store, &mut nodes, pulse, 0)?;
        store.reference(output)
    };
    let reference = if mode == "reference" {
        let b = producer(&mut store, &mut nodes, pulse, 100)?;
        let owner = NodeId(u32::try_from(nodes.len()).unwrap_or_else(|_| unreachable!()));
        let input = trigger(&mut store, owner, pulse)?;
        let out = store.add_shaped_output(owner, Kind::Reference(Box::new(shape())));
        nodes.push(slot(
            Route {
                input,
                out,
                sources: [source, store.reference(b)],
            },
            false,
        ));
        Some(out)
    } else {
        None
    };
    let owner = NodeId(u32::try_from(nodes.len()).unwrap_or_else(|_| unreachable!()));
    let input = store.add_shaped_input(owner, shape(), true);
    if let Some(r) = reference {
        store.follow(input, r, EngineTime::MIN_START, &mut Quiet)?;
    } else {
        store.sample(input, source, EngineTime::NEVER, &mut Quiet)?;
    }
    let mut inputs = Vec::new();
    for a in 0..2 {
        for b in 0..2 {
            let child = store.bindings().fixed_input(input, a);
            inputs.push(store.scalar_input(store.bindings().fixed_input(child, b))?);
        }
    }
    nodes.push(slot(
        Sink {
            inputs,
            sum: 0,
            count: 0,
        },
        false,
    ));
    Ok((store, Graph::new("fixed".into(), nodes), owner))
}
fn main() -> Result<(), Box<NodeError>> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "owned".into());
    if !["owned", "assembled", "reference"].contains(&mode.as_str()) {
        return Err(NodeError::new("expected owned, assembled or reference"));
    }
    let cycles = 200_000;
    let (mut store, mut graph, sink) =
        graph(&mode, cycles).map_err(|e| NodeError::new(format!("{e:?}")))?;
    let begin = Instant::now();
    run_simulation(
        &mut graph,
        &mut store,
        &RunConfig {
            start_time: EngineTime::MIN_START,
            end_time: EngineTime::from_micros(cycles + 8),
        },
    )
    .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let seconds = begin.elapsed().as_secs_f64();
    let sink = graph
        .node::<Sink>(sink)
        .ok_or_else(|| NodeError::new("missing sink"))?;
    let expected = 2 * cycles * (cycles - 1)
        + 6 * cycles
        + if mode == "reference" {
            400 * (cycles / 2)
        } else {
            0
        };
    assert_eq!(sink.sum, expected);
    assert_eq!(sink.count, cycles);
    #[expect(clippy::cast_precision_loss, reason = "benchmark time reporting only")]
    let ns = seconds * 1e9 / cycles as f64;
    println!(
        "{{\"ns_per_cycle\":{ns},\"checksum\":{},\"sink_evals\":{},\"ok\":true}}",
        sink.sum, sink.count
    );
    Ok(())
}
