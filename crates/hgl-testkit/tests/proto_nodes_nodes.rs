//! Each node alone, and each node in a small graph run by the engine, read
//! back through `Graph::node`. The graphs are the benchmark scenarios in
//! miniature: `pulse` as the source and `checksum` as the sink, with a
//! recording sink where the values or their times matter.

use std::fmt::Debug;

use hgl_describe::{BuildError, Buildable, Builder, Ports, Registry, instantiate};
use hgl_kernel::{Ctx, Graph, Node, NodeResult, RunConfig, run_simulation};
use hgl_store::{In, Out, Store};
use hgl_testkit::proto_nodes::{
    AddConst, AddOne, Checksum, ConfiguredSource, Constant41, Pulse, RunningSum, Shift, Sum,
    register_all,
};
use hgl_types::{
    EngineDelta, EngineTime, NodeId, NodeKind, NodeType, ScalarType, ScalarValue, TsType,
};

const START: EngineTime = EngineTime::MIN_START;
const I64: TsType = TsType::Ts(ScalarType::I64);

/// Far past the end of every graph here, so a run ends when nothing is
/// scheduled; a source that never stops fails on its count, not by hanging.
const STEPS: i64 = 1_000;

/// A sink that keeps what it is evaluated with, and when, in steps after the
/// start time.
struct Record {
    input: In<i64>,
    seen: Vec<(i64, i64)>,
}

impl Node for Record {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let step = ctx.evaluation_time().micros() - START.micros();
        self.seen.push((step, ctx.get(self.input)));
        Ok(())
    }
}

impl Buildable for Record {
    fn node_type() -> NodeType {
        NodeType {
            name: "record",
            inputs: vec![("in", I64)],
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            input: ports.input("in")?,
            seen: Vec::new(),
        })
    }
}

/// A source that emits `value` once, `at` steps after the start time: an
/// input that becomes valid later than the others.
struct Late {
    at: i64,
    value: i64,
    out: Out<i64>,
}

impl Node for Late {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(EngineDelta::from_micros(self.at))
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, self.value);
        Ok(())
    }
}

impl Buildable for Late {
    fn node_type() -> NodeType {
        NodeType {
            name: "late",
            output: Some(I64),
            scalars: vec![("at", ScalarType::I64), ("value", ScalarType::I64)],
            uses_scheduler: true,
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            at: ports.scalar("at")?,
            value: ports.scalar("value")?,
            out: ports.output()?,
        })
    }
}

fn registry() -> Result<Registry, BuildError> {
    let mut registry = Registry::new();
    register_all(&mut registry)?;
    registry.register::<Record>()?;
    registry.register::<Late>()?;
    Ok(registry)
}

fn scalar(name: &'static str, value: i64) -> [(&'static str, ScalarValue); 1] {
    [(name, ScalarValue::I64(value))]
}

fn failed(error: impl Debug) -> String {
    format!("{error:?}")
}

/// Describe, instantiate and run from `START` a graph of `nodes`, each an
/// implementation and its scalars, joined by `edges`, each (source, target,
/// input) by position in `nodes`. The nodes are given producers first, so
/// `finish` keeps their order and node `i` here is `NodeId(i)` in the graph.
/// Gives the graph, to read its sinks, and how many cycles ran.
fn run(
    nodes: &[(&str, &[(&str, ScalarValue)])],
    edges: &[(usize, usize, &str)],
) -> Result<(Graph, u64), String> {
    let registry = registry().map_err(failed)?;
    let mut builder = Builder::new("test", &registry);
    let mut refs = Vec::new();
    for &(implementation, scalars) in nodes {
        refs.push(builder.node(implementation, scalars).map_err(failed)?);
    }
    for &(source, target, input) in edges {
        let connected = builder.connect(refs[source], refs[target], input);
        connected.map_err(failed)?;
    }
    let description = builder.finish().map_err(failed)?;
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).map_err(failed)?;
    let config = RunConfig {
        start_time: START,
        end_time: EngineTime::from_micros(START.micros() + STEPS),
    };
    let cycles = run_simulation(&mut graph, &mut store, &config).map_err(failed)?;
    Ok((graph, cycles))
}

/// A checksum's (total, evaluations).
fn checksum(graph: &Graph, id: u32) -> Option<(u64, u64)> {
    let checksum = graph.node::<Checksum>(NodeId(id))?;
    Some((checksum.total(), checksum.evals()))
}

/// What a recording sink saw: (steps after the start, value). Nothing if
/// there is no recording sink at `id`.
fn seen(graph: &Graph, id: u32) -> Vec<(i64, i64)> {
    let record = graph.node::<Record>(NodeId(id));
    record.map(|record| record.seen.clone()).unwrap_or_default()
}

// Card, "Done when": `register_all` succeeds, and makes every node available
// under the name the cases and the twins use.
#[test]
fn register_all_registers_every_node_once() {
    let mut registry = Registry::new();
    assert_eq!(register_all(&mut registry), Ok(()));
    let names = [
        "add_one",
        "shift",
        "add_const",
        "sum",
        "running_sum",
        "constant_41",
        "configured_source",
        "pulse",
        "checksum",
    ];
    for name in names {
        assert_eq!(registry.node_type(name).map(|found| found.name), Some(name));
    }
    assert!(matches!(
        register_all(&mut registry),
        Err(BuildError::DuplicateImplementation(_))
    ));
}

/// Instantiate one `N`, alone, by its registered name, and run it: its kind,
/// once the graph is seen to hold an `N` at `NodeId(0)`.
fn alone<N: Buildable>(scalars: &[(&str, ScalarValue)]) -> Result<NodeKind, String> {
    let name = N::node_type().name;
    let (graph, _) = run(&[(name, scalars)], &[])?;
    let found = graph.node::<N>(NodeId(0));
    found.ok_or_else(|| format!("no {name} at NodeId(0)"))?;
    Ok(N::node_type().kind())
}

// Card, "Done when": each node instantiated alone, and its kind.
#[test]
fn add_one_alone_is_compute() {
    assert_eq!(alone::<AddOne>(&[]), Ok(NodeKind::Compute));
}

#[test]
fn shift_alone_is_compute() {
    assert_eq!(alone::<Shift>(&scalar("delta", 5)), Ok(NodeKind::Compute));
}

#[test]
fn add_const_alone_is_compute() {
    assert_eq!(alone::<AddConst>(&scalar("k", 5)), Ok(NodeKind::Compute));
}

#[test]
fn sum_alone_is_compute() {
    assert_eq!(alone::<Sum>(&[]), Ok(NodeKind::Compute));
}

#[test]
fn running_sum_alone_is_compute() {
    assert_eq!(alone::<RunningSum>(&[]), Ok(NodeKind::Compute));
}

#[test]
fn constant_41_alone_is_a_pull_source() {
    assert_eq!(alone::<Constant41>(&[]), Ok(NodeKind::PullSource));
}

#[test]
fn configured_source_alone_is_a_pull_source() {
    let value = scalar("value", 12);
    assert_eq!(alone::<ConfiguredSource>(&value), Ok(NodeKind::PullSource));
}

#[test]
fn pulse_alone_is_a_pull_source() {
    assert_eq!(
        alone::<Pulse>(&scalar("count", 3)),
        Ok(NodeKind::PullSource)
    );
}

#[test]
fn checksum_alone_is_a_sink() {
    assert_eq!(alone::<Checksum>(&[]), Ok(NodeKind::Sink));
}

// The baseline's `Pulse`: scheduled on start, it emits n and asks for the
// next step while n + 1 < count.
#[test]
fn pulse_emits_count_values_one_step_apart_from_the_start() {
    let (graph, cycles) = run(
        &[("pulse", &scalar("count", 5)), ("record", &[])],
        &[(0, 1, "in")],
    )
    .unwrap();
    assert_eq!(cycles, 5);
    assert_eq!(seen(&graph, 1), [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)]);
}

// As the baseline, a count of one or less still emits 0 once.
#[test]
fn pulse_of_one_or_none_emits_once() {
    for count in [1, 0] {
        let pulse = scalar("count", count);
        let (graph, cycles) = run(&[("pulse", &pulse), ("record", &[])], &[(0, 1, "in")]).unwrap();
        assert_eq!(cycles, 1);
        assert_eq!(seen(&graph, 1), [(0, 0)]);
    }
}

// The baseline's closed form for depth 0: the triangle, and one evaluation
// per cycle.
#[test]
fn pulse_into_checksum_matches_the_baseline() {
    let (graph, cycles) = run(
        &[("pulse", &scalar("count", 5)), ("checksum", &[])],
        &[(0, 1, "in")],
    )
    .unwrap();
    assert_eq!(cycles, 5);
    assert_eq!(checksum(&graph, 1), Some((10, 5)));
}

// The `tick` scenario: 0+1+2+3+4 and one more per cycle.
#[test]
fn add_one_adds_one() {
    let nodes = [
        ("pulse", &scalar("count", 5)[..]),
        ("add_one", &[]),
        ("checksum", &[]),
    ];
    let (graph, _) = run(&nodes, &[(0, 1, "in"), (1, 2, "in")]).unwrap();
    assert_eq!(checksum(&graph, 2), Some((15, 5)));
}

#[test]
fn add_const_adds_its_scalar() {
    let nodes = [
        ("pulse", &scalar("count", 5)[..]),
        ("add_const", &scalar("k", 7)),
        ("checksum", &[]),
    ];
    let (graph, _) = run(&nodes, &[(0, 1, "in"), (1, 2, "in")]).unwrap();
    assert_eq!(checksum(&graph, 2), Some((10 + 5 * 7, 5)));
}

#[test]
fn shift_adds_its_scalar() {
    let nodes = [
        ("pulse", &scalar("count", 3)[..]),
        ("shift", &scalar("delta", 5)),
        ("record", &[]),
    ];
    let (graph, _) = run(&nodes, &[(0, 1, "in"), (1, 2, "in")]).unwrap();
    assert_eq!(seen(&graph, 2), [(0, 5), (1, 6), (2, 7)]);
}

// Two pulses of different lengths, so that which input is read shows: once
// `rhs` stops at 2, `lhs` alone still wakes the sum.
#[test]
fn sum_adds_lhs_and_rhs() {
    let nodes = [
        ("pulse", &scalar("count", 5)[..]),
        ("pulse", &scalar("count", 3)),
        ("sum", &[]),
        ("checksum", &[]),
        ("record", &[]),
    ];
    let edges = [(0, 2, "lhs"), (1, 2, "rhs"), (2, 3, "in"), (2, 4, "in")];
    let (graph, _) = run(&nodes, &edges).unwrap();
    assert_eq!(seen(&graph, 4), [(0, 0), (1, 2), (2, 4), (3, 5), (4, 6)]);
    assert_eq!(checksum(&graph, 3), Some((17, 5)));
}

// NOD-2, with every input required and active, as the baseline's `add`:
// `sum` waits until `rhs` is valid, and then `rhs` alone wakes it. `lhs`
// ticks at 0, 1 and 2; `rhs` only at 4.
#[test]
fn nod2_sum_waits_for_both_inputs_then_either_wakes_it() {
    let late = [
        ("at", ScalarValue::I64(4)),
        ("value", ScalarValue::I64(100)),
    ];
    let nodes = [
        ("pulse", &scalar("count", 3)[..]),
        ("late", &late),
        ("sum", &[]),
        ("record", &[]),
    ];
    let edges = [(0, 2, "lhs"), (1, 2, "rhs"), (2, 3, "in")];
    let (graph, _) = run(&nodes, &edges).unwrap();
    assert_eq!(seen(&graph, 3), [(4, 102)]);
}

// NOD-9: the total is state, kept on the node from one cycle to the next.
#[test]
fn nod9_running_sum_keeps_its_total_between_cycles() {
    let nodes = [
        ("pulse", &scalar("count", 5)[..]),
        ("running_sum", &[]),
        ("record", &[]),
    ];
    let (graph, _) = run(&nodes, &[(0, 1, "in"), (1, 2, "in")]).unwrap();
    assert_eq!(seen(&graph, 2), [(0, 0), (1, 1), (2, 3), (3, 6), (4, 10)]);
}

#[test]
fn constant_41_ticks_once_at_the_start_time() {
    let (graph, cycles) = run(&[("constant_41", &[]), ("record", &[])], &[(0, 1, "in")]).unwrap();
    assert_eq!(cycles, 1);
    assert_eq!(seen(&graph, 1), [(0, 41)]);
}

#[test]
fn configured_source_ticks_its_value_once_at_the_start_time() {
    let value = scalar("value", 12);
    let (graph, cycles) = run(
        &[("configured_source", &value), ("record", &[])],
        &[(0, 1, "in")],
    )
    .unwrap();
    assert_eq!(cycles, 1);
    assert_eq!(seen(&graph, 1), [(0, 12)]);
}

// As the baseline's `static_cast<U64>` and unsigned `+=`: the bits of a
// negative value, and a total that wraps. 2^63 + (2^63 + 1) is 1.
#[test]
fn checksum_wraps_as_the_baselines_unsigned_total() {
    let nodes = [
        ("pulse", &scalar("count", 2)[..]),
        ("add_const", &scalar("k", i64::MIN)),
        ("checksum", &[]),
    ];
    let (graph, _) = run(&nodes, &[(0, 1, "in"), (1, 2, "in")]).unwrap();
    assert_eq!(checksum(&graph, 2), Some((1, 2)));
}

// NOD-5: `add_one` reads the pulse and writes only its own output; the pulse's
// output still shows the pulse.
#[test]
fn nod5_a_node_writes_only_its_own_output() {
    let nodes = [
        ("pulse", &scalar("count", 3)[..]),
        ("add_one", &[]),
        ("record", &[]),
        ("checksum", &[]),
    ];
    let (graph, _) = run(&nodes, &[(0, 1, "in"), (0, 2, "in"), (1, 3, "in")]).unwrap();
    assert_eq!(seen(&graph, 2), [(0, 0), (1, 1), (2, 2)]);
    assert_eq!(checksum(&graph, 3), Some((6, 3)));
}

// INJ-2, INJ-4: only `pulse` uses a scheduler, so only its type asks for one;
// the sources that tick once ask only to be scheduled on start.
#[test]
fn inj2_only_pulse_asks_for_a_scheduler() {
    let types = [
        AddOne::node_type(),
        Shift::node_type(),
        AddConst::node_type(),
        Sum::node_type(),
        RunningSum::node_type(),
        Constant41::node_type(),
        ConfiguredSource::node_type(),
        Pulse::node_type(),
        Checksum::node_type(),
    ];
    let sources = ["constant_41", "configured_source", "pulse"];
    for node_type in types {
        assert_eq!(node_type.uses_scheduler, node_type.name == "pulse");
        assert_eq!(
            node_type.schedule_on_start,
            sources.contains(&node_type.name),
            "{}",
            node_type.name
        );
    }
}

// Card, Surface: each node's inputs in order, its output and its scalars —
// the names the cases and the twins wire by — and, as the baseline's static
// nodes, every input active and required.
#[test]
fn signatures_are_the_cards() {
    let i64s = ScalarType::I64;
    let signatures = [
        (AddOne::node_type(), vec![("in", I64)], Some(I64), vec![]),
        (
            Shift::node_type(),
            vec![("in", I64)],
            Some(I64),
            vec![("delta", i64s)],
        ),
        (
            AddConst::node_type(),
            vec![("in", I64)],
            Some(I64),
            vec![("k", i64s)],
        ),
        (
            Sum::node_type(),
            vec![("lhs", I64), ("rhs", I64)],
            Some(I64),
            vec![],
        ),
        (
            RunningSum::node_type(),
            vec![("in", I64)],
            Some(I64),
            vec![],
        ),
        (Constant41::node_type(), vec![], Some(I64), vec![]),
        (
            ConfiguredSource::node_type(),
            vec![],
            Some(I64),
            vec![("value", i64s)],
        ),
        (Pulse::node_type(), vec![], Some(I64), vec![("count", i64s)]),
        (Checksum::node_type(), vec![("in", I64)], None, vec![]),
    ];
    for (node_type, inputs, output, scalars) in signatures {
        let name = node_type.name;
        assert_eq!(node_type.inputs, inputs, "{name}");
        assert_eq!(node_type.output, output, "{name}");
        assert_eq!(node_type.scalars, scalars, "{name}");
        assert_eq!(node_type.active_inputs, None, "{name}");
        assert_eq!(node_type.valid_inputs, None, "{name}");
    }
}
