//! A description brought to life: what instantiation resolves, what it
//! refuses, and what the graph it leaves does when run (specification:
//! Graph, Part 1, "Behaviour").

use hgl_describe::{
    BuildError, Buildable, Builder, Edge, GraphDescription, NodeDescription, Ports, Registry,
    instantiate,
};
use hgl_kernel::{Ctx, EngineError, Graph, Node, NodeError, NodeResult, Phase, RunConfig};
use hgl_store::{In, InputId, Out, OutputId, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType, ScalarType, ScalarValue, TsType};

const I64: TsType = TsType::Ts(ScalarType::I64);
const F64: TsType = TsType::Ts(ScalarType::F64);

/// A node type whose inputs and scalars are all `i64`.
fn signature(
    name: &'static str,
    inputs: &[&'static str],
    output: Option<TsType>,
    scalars: &[&'static str],
) -> NodeType {
    NodeType {
        name,
        inputs: inputs.iter().map(|&input| (input, I64)).collect(),
        output,
        scalars: scalars
            .iter()
            .map(|&scalar| (scalar, ScalarType::I64))
            .collect(),
        active_inputs: None,
        valid_inputs: None,
        uses_scheduler: false,
        schedule_on_start: false,
        child_graphs: 0,
    }
}

/// Emits `first`, `first + 1`, ... from the start time, once every `every`
/// microseconds.
struct Source {
    out: Out<i64>,
    next: i64,
    every: i64,
}

impl Node for Source {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, self.next);
        self.next += 1;
        ctx.schedule_in(EngineDelta::from_micros(self.every))
    }
}

impl Buildable for Source {
    fn node_type() -> NodeType {
        NodeType {
            uses_scheduler: true,
            schedule_on_start: true,
            ..signature("source", &[], Some(I64), &["first", "every"])
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            out: ports.output()?,
            next: ports.scalar("first")?,
            every: ports.scalar("every")?,
        })
    }
}

/// `lhs - rhs`: which input is which shows.
struct Difference {
    lhs: In<i64>,
    rhs: In<i64>,
    out: Out<i64>,
}

impl Node for Difference {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.lhs) - ctx.get(self.rhs));
        Ok(())
    }
}

impl Buildable for Difference {
    fn node_type() -> NodeType {
        signature("difference", &["lhs", "rhs"], Some(I64), &[])
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            lhs: ports.input("lhs")?,
            rhs: ports.input("rhs")?,
            out: ports.output()?,
        })
    }
}

/// `x * scale + offset`: which scalar is which shows.
struct Affine {
    x: In<i64>,
    scale: i64,
    offset: i64,
    out: Out<i64>,
}

impl Node for Affine {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.x) * self.scale + self.offset);
        Ok(())
    }
}

impl Buildable for Affine {
    fn node_type() -> NodeType {
        signature("affine", &["x"], Some(I64), &["scale", "offset"])
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            x: ports.input("x")?,
            scale: ports.scalar("scale")?,
            offset: ports.scalar("offset")?,
            out: ports.output()?,
        })
    }
}

/// A sink that keeps every value it is evaluated with.
struct Record {
    input: In<i64>,
    seen: Vec<i64>,
}

impl Node for Record {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.seen.push(ctx.get(self.input));
        Ok(())
    }
}

impl Buildable for Record {
    fn node_type() -> NodeType {
        signature("record", &["in"], None, &[])
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            input: ports.input("in")?,
            seen: Vec::new(),
        })
    }
}

/// Woken by `trigger` only, and requires only `trigger`: keeps what `value`
/// shows each time, `None` while it is not valid. `trigger` is second, so
/// that its position is not the first.
struct Sample {
    value: In<i64>,
    seen: Vec<Option<i64>>,
}

impl Node for Sample {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let value = ctx.valid(self.value).then(|| ctx.get(self.value));
        self.seen.push(value);
        Ok(())
    }
}

impl Buildable for Sample {
    fn node_type() -> NodeType {
        NodeType {
            active_inputs: Some(vec![1]),
            valid_inputs: Some(vec![1]),
            ..signature("sample", &["value", "trigger"], None, &[])
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        let _trigger: In<i64> = ports.input("trigger")?;
        Ok(Self {
            value: ports.input("value")?,
            seen: Vec::new(),
        })
    }
}

/// Counts its evaluations. It reads neither input, so its build asks for
/// neither: `trigger`, the second, wakes it, and `value` does not.
struct Count {
    out: Out<i64>,
    count: i64,
}

impl Node for Count {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.count += 1;
        ctx.set(self.out, self.count);
        Ok(())
    }
}

impl Buildable for Count {
    fn node_type() -> NodeType {
        NodeType {
            active_inputs: Some(vec![1]),
            ..signature("count", &["value", "trigger"], Some(I64), &[])
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            out: ports.output()?,
            count: 0,
        })
    }
}

/// A `bool` and an `f64` input, a `bool` output, and asks for none of them.
struct Idle;

impl Node for Idle {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

impl Buildable for Idle {
    fn node_type() -> NodeType {
        let inputs = vec![("flag", TsType::Ts(ScalarType::Bool)), ("price", F64)];
        NodeType {
            inputs,
            output: Some(TsType::Ts(ScalarType::Bool)),
            ..signature("idle", &[], None, &[])
        }
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

/// `Idle`'s inputs, an `f64` output, and asks for every one of them by its
/// type.
struct Busy;

impl Node for Busy {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

impl Buildable for Busy {
    fn node_type() -> NodeType {
        NodeType {
            name: "busy",
            output: Some(F64),
            ..Idle::node_type()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        ports.input::<bool>("flag")?;
        ports.input::<f64>("price")?;
        ports.output::<f64>()?;
        Ok(Self)
    }
}

/// Counts its evaluations. It takes neither its input nor, therefore, any
/// say over it: the node type leaves activity and validity at their
/// defaults, so `trigger` is active and required.
struct Tally {
    out: Out<i64>,
    count: i64,
}

impl Node for Tally {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.count += 1;
        ctx.set(self.out, self.count);
        Ok(())
    }
}

impl Buildable for Tally {
    fn node_type() -> NodeType {
        signature("tally", &["trigger"], Some(I64), &[])
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            out: ports.output()?,
            count: 0,
        })
    }
}

/// Declares an output its build never asks for.
struct Silent;

impl Node for Silent {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

impl Buildable for Silent {
    fn node_type() -> NodeType {
        signature("silent", &[], Some(I64), &[])
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

/// Its output is not of its input's type.
struct ToFloat;

impl Node for ToFloat {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

impl Buildable for ToFloat {
    fn node_type() -> NodeType {
        signature("to_float", &["in"], Some(F64), &[])
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

/// Fails to start.
struct Fail;

impl Node for Fail {
    fn start(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Err(NodeError::new("does not start"))
    }
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

impl Buildable for Fail {
    fn node_type() -> NodeType {
        signature("fail", &[], None, &[])
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

/// A build that gets one thing wrong, chosen by its `fault` scalar; none
/// from 7 on.
struct Misbuilt;

impl Node for Misbuilt {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

impl Buildable for Misbuilt {
    fn node_type() -> NodeType {
        signature("misbuilt", &["x"], Some(I64), &["fault"])
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        match ports.scalar::<i64>("fault")? {
            0 => drop(ports.input::<i64>("y")?),
            1 => drop(ports.input::<f64>("x")?),
            2 => drop((ports.input::<i64>("x")?, ports.input::<i64>("x")?)),
            3 => drop(ports.output::<f64>()?),
            4 => drop((ports.output::<i64>()?, ports.output::<i64>()?)),
            5 => drop(ports.scalar::<i64>("unknown")?),
            6 => drop(ports.scalar::<f64>("fault")?),
            _ => {}
        }
        Ok(Self)
    }
}

/// A sink whose build asks for an output.
struct GreedySink;

impl Node for GreedySink {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

impl Buildable for GreedySink {
    fn node_type() -> NodeType {
        signature("greedy_sink", &["in"], None, &[])
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        ports.output::<i64>()?;
        Ok(Self)
    }
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    assert_eq!(registry.register::<Source>(), Ok(()));
    assert_eq!(registry.register::<Difference>(), Ok(()));
    assert_eq!(registry.register::<Affine>(), Ok(()));
    assert_eq!(registry.register::<Record>(), Ok(()));
    assert_eq!(registry.register::<Sample>(), Ok(()));
    assert_eq!(registry.register::<Count>(), Ok(()));
    assert_eq!(registry.register::<Silent>(), Ok(()));
    assert_eq!(registry.register::<Idle>(), Ok(()));
    assert_eq!(registry.register::<Busy>(), Ok(()));
    assert_eq!(registry.register::<Tally>(), Ok(()));
    assert_eq!(registry.register::<ToFloat>(), Ok(()));
    assert_eq!(registry.register::<Fail>(), Ok(()));
    assert_eq!(registry.register::<Misbuilt>(), Ok(()));
    assert_eq!(registry.register::<GreedySink>(), Ok(()));
    registry
}

fn source(first: i64, every: i64) -> [(&'static str, ScalarValue); 2] {
    [
        ("first", ScalarValue::I64(first)),
        ("every", ScalarValue::I64(every)),
    ]
}

/// Run from the earliest start for `cycles` microseconds.
fn run(graph: &mut Graph, store: &mut Store, cycles: i64) -> Result<u64, EngineError> {
    let end_time = EngineTime::from_micros(EngineTime::MIN_START.micros() + cycles);
    let config = RunConfig {
        start_time: EngineTime::MIN_START,
        end_time,
    };
    hgl_kernel::run_simulation(graph, store, &config)
}

/// Whether nothing was ever added to `store`: the next output and input it
/// makes are its first.
fn nothing_added(mut store: Store) -> bool {
    store.add_output::<i64>(NodeId(0)).id() == OutputId(0)
        && store.add_input::<i64>(NodeId(0), true).id() == InputId(0)
}

/// A description written out by hand, as one loaded from a file would be.
/// Each node is labelled with its implementation.
fn written(
    nodes: &[(&str, &[(&str, ScalarValue)])],
    edges: &[(u32, u32, u32)],
) -> GraphDescription {
    GraphDescription {
        label: "written".to_owned(),
        nodes: nodes
            .iter()
            .map(|&(implementation, scalars)| NodeDescription {
                children: Vec::new(),
                implementation: implementation.to_owned(),
                label: implementation.to_owned(),
                scalars: scalars
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), value.clone()))
                    .collect(),
            })
            .collect(),
        edges: edges
            .iter()
            .map(|&(source_node, target_node, target_input)| Edge {
                source: hgl_describe::OutputPort {
                    node: source_node,
                    path: vec![],
                },
                target: hgl_describe::InputPort {
                    node: target_node,
                    input: target_input,
                    path: vec![],
                },
            })
            .collect(),
    }
}

/// `source(10, 1) -> affine(2, 1) -> record`, added consumer first.
fn chain(registry: &Registry) -> Result<GraphDescription, BuildError> {
    let mut builder = Builder::new("chain", registry);
    let record = builder.node("record", &[])?;
    let scalars = [
        ("scale", ScalarValue::I64(2)),
        ("offset", ScalarValue::I64(1)),
    ];
    let affine = builder.node("affine", &scalars)?;
    let source = builder.node("source", &source(10, 1))?;
    builder.connect(affine, record, "in")?;
    builder.connect(source, affine, "x")?;
    builder.finish()
}

/// Whether `description` would fail to instantiate with `expected`, and add
/// nothing to the store on the way.
fn refused(description: &GraphDescription, expected: BuildError) {
    let mut store = Store::new();
    let outcome = instantiate(description, &registry(), &mut store);
    assert_eq!(outcome.err(), Some(expected));
    assert!(nothing_added(store), "a refused description adds nothing");
}

// GRF-3; card, Surface: the node at `description.nodes[i]` is `NodeId(i)` in
// the graph, and owns the store handles it was built with — its writes pass
// the store's owner check (TS-21, debug) and its watchers wake it.
#[test]
fn grf3_node_i_of_the_description_is_node_id_i() {
    let registry = registry();
    let description = chain(&registry).unwrap();
    let implementations: Vec<&str> = description
        .nodes
        .iter()
        .map(|node| node.implementation.as_str())
        .collect();
    assert_eq!(implementations, ["source", "affine", "record"]);

    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();

    assert!(graph.node::<Source>(NodeId(0)).is_some());
    assert!(graph.node::<Affine>(NodeId(1)).is_some());
    assert!(graph.node::<Record>(NodeId(2)).is_some());
    assert!(graph.node::<Record>(NodeId(0)).is_none());
    assert!(graph.node::<Record>(NodeId(3)).is_none());
    assert_eq!(run(&mut graph, &mut store, 3), Ok(3));
    assert_eq!(graph.node::<Record>(NodeId(2)).unwrap().seen, [21, 23, 25]);
}

// GRF-3: a node's label and position, and the graph's label, reach the
// errors the running graph reports. The graph's own is the kernel's GRF-20
// error, for a second start.
#[test]
fn grf3_a_graph_and_its_nodes_are_reported_by_their_description() {
    let mut description = written(&[("silent", &[]), ("fail", &[])], &[]);
    description.nodes[1].label = "doomed".to_owned();
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry(), &mut store).unwrap();

    let Err(EngineError::Node(error)) = run(&mut graph, &mut store, 1) else {
        panic!("the second node fails to start");
    };
    assert_eq!((error.node, error.label.as_str()), (NodeId(1), "doomed"));
    assert_eq!(error.phase, Phase::Start);
    let again = graph.start(&mut store, EngineTime::MIN_START).unwrap_err();
    assert_eq!(again.label, "written");
}

// GRF-2; card, "Done when": the same description instantiated twice gives
// two independent graphs that tick identically. They share one store and no
// slot in it: running one ticks nothing of the other.
#[test]
fn grf2_two_graphs_from_one_description_are_independent_and_tick_identically() {
    let registry = registry();
    let description = chain(&registry).unwrap();
    let mut store = Store::new();
    let mut first = instantiate(&description, &registry, &mut store).unwrap();
    let mut second = instantiate(&description, &registry, &mut store).unwrap();

    assert_eq!(run(&mut second, &mut store, 3), Ok(3));
    assert_eq!(second.node::<Record>(NodeId(2)).unwrap().seen, [21, 23, 25]);
    let untouched = first.node::<Source>(NodeId(0)).unwrap().out;
    assert_eq!(store.output_value_erased(untouched.id()), None);

    assert_eq!(run(&mut first, &mut store, 3), Ok(3));
    assert_eq!(
        first.node::<Record>(NodeId(2)).unwrap().seen,
        second.node::<Record>(NodeId(2)).unwrap().seen
    );
}

// GRF-1; card, "Done when": the description compares equal to a copy of
// itself after instantiation. (`instantiate` borrows it shared, and it has
// no interior mutability, so this is also held by the signature.)
#[test]
fn grf1_instantiating_leaves_the_description_as_it_was() {
    let registry = registry();
    let description = chain(&registry).unwrap();
    let copy = description.clone();
    let mut store = Store::new();
    instantiate(&description, &registry, &mut store).unwrap();
    instantiate(&description, &registry, &mut store).unwrap();
    assert_eq!(description, copy);
}

// GRF-9; card, "Done when": an unresolved implementation yields no graph at
// all — not one with that node left out — and every implementation is
// resolved before anything is made.
#[test]
fn grf9_an_unknown_implementation_yields_no_graph() {
    let description = written(
        &[("source", &source(0, 1)), ("nothing", &[]), ("record", &[])],
        &[(0, 2, 0)],
    );
    refused(
        &description,
        BuildError::UnknownImplementation("nothing".to_owned()),
    );
}

// GRF-9: a node whose build fails yields no graph either. The graph is all
// or nothing; the store is not yet — the ports made for the source, and any
// the failing node took, stay in it, unbound and unreachable. Rolling them
// back waits for P4 (card), so this test pins the error and not the store.
#[test]
fn grf9_a_build_that_fails_yields_no_graph() {
    let fault = [("fault", ScalarValue::I64(0))];
    let description = written(&[("source", &source(0, 1)), ("misbuilt", &fault)], &[]);
    let mut store = Store::new();
    let outcome = instantiate(&description, &registry(), &mut store);
    let unknown_input = BuildError::UnknownInput {
        node: "misbuilt".to_owned(),
        input: "y".to_owned(),
    };
    assert_eq!(outcome.err(), Some(unknown_input));
}

// Card, "Done when": an unknown input or scalar and a wrong type are each
// rejected with the matching error when a build asks for them; so is taking
// an input or the output twice, or an output the node type does not
// declare.
#[test]
fn ports_refuse_what_the_node_type_does_not_declare() {
    let misbuilt = |fault: i64| {
        let description = written(&[("misbuilt", &[("fault", ScalarValue::I64(fault))])], &[]);
        instantiate(&description, &registry(), &mut Store::new()).err()
    };
    let node = || "misbuilt".to_owned();
    let wrong_type = |what: &str| BuildError::WrongType {
        node: node(),
        what: what.to_owned(),
    };

    let unknown_input = BuildError::UnknownInput {
        node: node(),
        input: "y".to_owned(),
    };
    assert_eq!(misbuilt(0), Some(unknown_input));
    assert_eq!(misbuilt(1), Some(wrong_type("x")));
    let taken_twice = BuildError::InputBoundTwice {
        node: node(),
        input: "x".to_owned(),
    };
    assert_eq!(misbuilt(2), Some(taken_twice));
    assert_eq!(misbuilt(3), Some(wrong_type("output")));
    assert_eq!(misbuilt(4), Some(BuildError::NoOutput { node: node() }));
    let unknown_scalar = BuildError::UnknownScalar {
        node: node(),
        scalar: "unknown".to_owned(),
    };
    assert_eq!(misbuilt(5), Some(unknown_scalar));
    assert_eq!(misbuilt(6), Some(wrong_type("fault")));
    assert_eq!(misbuilt(7), None, "a build that asks for what it declares");

    let description = written(&[("greedy_sink", &[])], &[]);
    let outcome = instantiate(&description, &registry(), &mut Store::new());
    let no_output = BuildError::NoOutput {
        node: "greedy_sink".to_owned(),
    };
    assert_eq!(outcome.err(), Some(no_output));
}

// GRF-4: in a description the order is the rank order, so an edge that does
// not run forward in it is refused.
#[test]
fn grf4_an_edge_that_does_not_run_forward_is_refused() {
    let scalars = [
        ("scale", ScalarValue::I64(1)),
        ("offset", ScalarValue::I64(0)),
    ];
    let backward = written(
        &[("affine", &scalars), ("source", &source(0, 1))],
        &[(1, 0, 0)],
    );
    refused(&backward, BuildError::Cycle);
    let to_itself = written(&[("affine", &scalars)], &[(0, 0, 0)]);
    refused(&to_itself, BuildError::Cycle);
}

// GRF-6: a description that binds one input twice is refused.
#[test]
fn grf6_a_description_that_binds_an_input_twice_is_refused() {
    let description = written(
        &[
            ("source", &source(0, 1)),
            ("source", &source(5, 1)),
            ("record", &[]),
        ],
        &[(0, 2, 0), (1, 2, 0)],
    );
    let expected = BuildError::InputBoundTwice {
        node: "record".to_owned(),
        input: "in".to_owned(),
    };
    refused(&description, expected);
}

// GRF-7: loading a stored description must establish that its edges join
// the same types again.
#[test]
fn grf7_a_description_that_joins_different_types_is_refused() {
    let description = written(
        &[
            ("source", &source(0, 1)),
            ("to_float", &[]),
            ("record", &[]),
        ],
        &[(0, 1, 0), (1, 2, 0)],
    );
    let expected = BuildError::WrongType {
        node: "record".to_owned(),
        what: "in".to_owned(),
    };
    refused(&description, expected);
}

// GRF-8: a description whose scalars do not conform is refused.
#[test]
fn grf8_a_description_whose_scalars_do_not_conform_is_refused() {
    let wrong = [
        ("first", ScalarValue::F64(0.0)),
        ("every", ScalarValue::I64(1)),
    ];
    let expected = BuildError::WrongType {
        node: "source".to_owned(),
        what: "first".to_owned(),
    };
    // The good node comes first: its ports would already be in the store if
    // the scalars were checked as each node is built.
    let nodes: &[(&str, &[(&str, ScalarValue)])] = &[("silent", &[]), ("source", &wrong)];
    refused(&written(nodes, &[]), expected);

    let missing = [("first", ScalarValue::I64(0))];
    let expected = BuildError::UnknownScalar {
        node: "source".to_owned(),
        scalar: "every".to_owned(),
    };
    refused(&written(&[("source", &missing)], &[]), expected);
}

// An edge must name a node and an input that exist, from a node with an
// output.
#[test]
fn an_edge_to_or_from_what_does_not_exist_is_refused() {
    let nodes: &[(&str, &[(&str, ScalarValue)])] = &[("source", &source(0, 1)), ("record", &[])];
    let no_node = BuildError::UnknownInput {
        node: "5".to_owned(),
        input: "0".to_owned(),
    };
    refused(&written(nodes, &[(0, 5, 0)]), no_node);
    let one_past_the_end = BuildError::UnknownInput {
        node: "2".to_owned(),
        input: "0".to_owned(),
    };
    refused(&written(nodes, &[(0, 2, 0)]), one_past_the_end);
    let no_input = BuildError::UnknownInput {
        node: "record".to_owned(),
        input: "3".to_owned(),
    };
    refused(&written(nodes, &[(0, 1, 3)]), no_input);

    let from_a_sink = written(&[("record", &[]), ("record", &[])], &[(0, 1, 0)]);
    let no_output = BuildError::NoOutput {
        node: "record".to_owned(),
    };
    refused(&from_a_sink, no_output);
}

// Card, Surface: an edge binds the input it names. `lhs - rhs` shows which.
#[test]
fn an_edge_binds_the_input_it_names() {
    let registry = registry();
    let mut builder = Builder::new("difference", &registry);
    let lhs = builder.node("source", &source(10, 1)).unwrap();
    let rhs = builder.node("source", &source(1, 1)).unwrap();
    let difference = builder.node("difference", &[]).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(rhs, difference, "rhs").unwrap();
    builder.connect(lhs, difference, "lhs").unwrap();
    builder.connect(difference, record, "in").unwrap();
    let description = builder.finish().unwrap();
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();

    assert_eq!(run(&mut graph, &mut store, 3), Ok(3));
    assert_eq!(graph.node::<Record>(NodeId(3)).unwrap().seen, [9, 9, 9]);
}

// GRF-8: a node reads the scalar it names, whatever order they are given in.
#[test]
fn grf8_a_node_reads_the_scalar_it_names() {
    let registry = registry();
    let mut builder = Builder::new("affine", &registry);
    let source = builder.node("source", &source(1, 1)).unwrap();
    let scalars = [
        ("offset", ScalarValue::I64(5)),
        ("scale", ScalarValue::I64(3)),
    ];
    let affine = builder.node("affine", &scalars).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(source, affine, "x").unwrap();
    builder.connect(affine, record, "in").unwrap();
    let description = builder.finish().unwrap();
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();

    assert_eq!(run(&mut graph, &mut store, 3), Ok(3));
    assert_eq!(graph.node::<Record>(NodeId(2)).unwrap().seen, [8, 11, 14]);
}

// NOD-4, GRF-17: an input is active only if the node type says so. `sample`
// is woken by its trigger, every other cycle, and not by its value, which
// ticks every cycle.
#[test]
fn nod4_grf17_only_the_inputs_the_node_type_makes_active_wake_it() {
    let registry = registry();
    let mut builder = Builder::new("sample", &registry);
    let trigger = builder.node("source", &source(0, 2)).unwrap();
    let value = builder.node("source", &source(100, 1)).unwrap();
    let sample = builder.node("sample", &[]).unwrap();
    builder.connect(trigger, sample, "trigger").unwrap();
    builder.connect(value, sample, "value").unwrap();
    let description = builder.finish().unwrap();
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();

    assert_eq!(run(&mut graph, &mut store, 6), Ok(6));
    let seen = &graph.node::<Sample>(NodeId(2)).unwrap().seen;
    assert_eq!(seen, &[Some(100), Some(102), Some(104)]);
}

// NOD-2, GRF-6: the node type's valid inputs become the inputs the kernel
// requires. `sample` requires only its trigger, so it runs while its value,
// the target of no edge, stays unbound and not valid; `difference` requires
// both, so it never runs without its `rhs`.
#[test]
fn nod2_grf6_valid_inputs_become_the_required_inputs() {
    let registry = registry();
    let mut builder = Builder::new("unbound", &registry);
    let trigger = builder.node("source", &source(0, 1)).unwrap();
    let sample = builder.node("sample", &[]).unwrap();
    let difference = builder.node("difference", &[]).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(trigger, sample, "trigger").unwrap();
    builder.connect(trigger, difference, "lhs").unwrap();
    builder.connect(difference, record, "in").unwrap();
    let description = builder.finish().unwrap();
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();

    assert_eq!(run(&mut graph, &mut store, 3), Ok(3));
    let seen = &graph.node::<Sample>(NodeId(1)).unwrap().seen;
    assert_eq!(seen, &[None, None, None]);
    assert_eq!(graph.node::<Record>(NodeId(3)).unwrap().seen, []);
}

// NOD-4, GRF-17: a port the node type declares is made whether or not the
// build asks for it, active or passive as the node type says. `count` takes
// neither input: `trigger` still wakes it every other cycle, and `value`,
// which ticks every cycle, does not. `silent` never takes its output, which
// can still be bound and never ticks.
#[test]
fn nod4_a_port_the_build_does_not_ask_for_is_still_made() {
    let registry = registry();
    let mut builder = Builder::new("untaken", &registry);
    let trigger = builder.node("source", &source(0, 2)).unwrap();
    let value = builder.node("source", &source(100, 1)).unwrap();
    let count = builder.node("count", &[]).unwrap();
    let counted = builder.node("record", &[]).unwrap();
    let silent = builder.node("silent", &[]).unwrap();
    let quiet = builder.node("record", &[]).unwrap();
    builder.connect(trigger, count, "trigger").unwrap();
    builder.connect(value, count, "value").unwrap();
    builder.connect(count, counted, "in").unwrap();
    builder.connect(silent, quiet, "in").unwrap();
    let description = builder.finish().unwrap();
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();

    assert_eq!(run(&mut graph, &mut store, 6), Ok(6));
    assert_eq!(graph.node::<Record>(NodeId(5)).unwrap().seen, [1, 2, 3]);
    assert_eq!(graph.node::<Record>(NodeId(4)).unwrap().seen, []);
}

// GRF-7: a port the build does not ask for is made with the type its node
// type declares, so it binds to a port a build asked for by that type, of
// each scalar type, in each direction.
#[test]
fn grf7_a_port_the_build_does_not_ask_for_has_its_declared_type() {
    let registry = registry();
    let mut builder = Builder::new("idle", &registry);
    let count = builder.node("source", &source(0, 1)).unwrap();
    let price = builder.node("to_float", &[]).unwrap();
    let first = builder.node("idle", &[]).unwrap();
    let busy = builder.node("busy", &[]).unwrap();
    let second = builder.node("idle", &[]).unwrap();
    builder.connect(count, price, "in").unwrap();
    builder.connect(price, busy, "price").unwrap();
    builder.connect(first, busy, "flag").unwrap();
    builder.connect(busy, second, "price").unwrap();
    builder.connect(first, second, "flag").unwrap();
    let description = builder.finish().unwrap();

    let mut store = Store::new();
    assert!(instantiate(&description, &registry, &mut store).is_ok());
}

// NOD-4, GRF-17: an untaken input of a node type that leaves `active_inputs`
// at its default wakes its node, as a taken one would.
#[test]
fn nod4_an_untaken_input_is_active_by_default() {
    let registry = registry();
    let mut builder = Builder::new("default active", &registry);
    let trigger = builder.node("source", &source(0, 1)).unwrap();
    let tally = builder.node("tally", &[]).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(trigger, tally, "trigger").unwrap();
    builder.connect(tally, record, "in").unwrap();
    let description = builder.finish().unwrap();
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();

    assert_eq!(run(&mut graph, &mut store, 3), Ok(3));
    assert_eq!(graph.node::<Record>(NodeId(2)).unwrap().seen, [1, 2, 3]);
}

// NOD-2, GRF-6: an untaken input of a node type that leaves `valid_inputs`
// at its default is required, so a node whose untaken input no edge binds is
// never evaluated, however often its other input wakes it.
#[test]
fn nod2_an_untaken_input_is_required_by_default() {
    let registry = registry();
    let mut builder = Builder::new("default required", &registry);
    let trigger = builder.node("source", &source(0, 1)).unwrap();
    let count = builder.node("count", &[]).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(trigger, count, "trigger").unwrap();
    builder.connect(count, record, "in").unwrap();
    let description = builder.finish().unwrap();
    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();

    assert_eq!(run(&mut graph, &mut store, 3), Ok(3));
    assert_eq!(graph.node::<Record>(NodeId(2)).unwrap().seen, []);
}

// GRF-8: a stored description is checked against the node type whether or
// not that type declares scalars: a node type with none takes none.
#[test]
fn grf8_a_stored_scalar_is_refused_by_a_node_type_that_declares_none() {
    let extra: &[(&str, ScalarValue)] = &[("extra", ScalarValue::I64(1))];
    let nodes: &[(&str, &[(&str, ScalarValue)])] = &[("silent", &[]), ("record", extra)];
    let expected = BuildError::UnknownScalar {
        node: "record".to_owned(),
        scalar: "extra".to_owned(),
    };
    refused(&written(nodes, &[]), expected);
}
