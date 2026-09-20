//! Writing a description by hand: what the builder checks as each node and
//! edge is added, and how `finish` ranks (specification: Graph, Part 1).
//!
//! The nodes here are shapes only: nothing in this file instantiates them.

use hgl_describe::{
    BuildError, Buildable, Builder, Edge, GraphDescription, NodeRef, Ports, Registry,
};
use hgl_kernel::{Ctx, Node, NodeResult};
use hgl_types::{NodeType, ScalarType, ScalarValue, TsType};

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
    }
}

struct Source;
struct Difference;
struct Record;
struct ToFloat;
/// Its node type takes a name `Source` already has.
struct Impostor;
/// Two inputs cannot be told apart by name.
struct TwoOfOneName;
/// `active_inputs` names the position after its last input.
struct ActivePastTheEnd;
/// `valid_inputs` names the position after its last input.
struct ValidPastTheEnd;

impl Node for Source {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for Source {
    fn node_type() -> NodeType {
        signature("source", &[], Some(I64), &["first"])
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

impl Node for Difference {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for Difference {
    fn node_type() -> NodeType {
        signature("difference", &["lhs", "rhs"], Some(I64), &[])
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

impl Node for Record {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for Record {
    fn node_type() -> NodeType {
        signature("record", &["in"], None, &[])
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

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

impl Node for Impostor {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for Impostor {
    fn node_type() -> NodeType {
        signature("source", &["in"], None, &[])
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

impl Node for TwoOfOneName {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for TwoOfOneName {
    fn node_type() -> NodeType {
        signature("two_of_one_name", &["in", "in"], None, &[])
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

impl Node for ActivePastTheEnd {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for ActivePastTheEnd {
    fn node_type() -> NodeType {
        NodeType {
            active_inputs: Some(vec![1]),
            ..signature("active_past_the_end", &["in"], None, &[])
        }
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

impl Node for ValidPastTheEnd {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
impl Buildable for ValidPastTheEnd {
    fn node_type() -> NodeType {
        NodeType {
            valid_inputs: Some(vec![1]),
            ..signature("valid_past_the_end", &["in"], None, &[])
        }
    }
    fn build(_ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self)
    }
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    assert_eq!(registry.register::<Source>(), Ok(()));
    assert_eq!(registry.register::<Difference>(), Ok(()));
    assert_eq!(registry.register::<Record>(), Ok(()));
    assert_eq!(registry.register::<ToFloat>(), Ok(()));
    registry
}

fn first(value: i64) -> [(&'static str, ScalarValue); 1] {
    [("first", ScalarValue::I64(value))]
}

/// Each node of a description as its implementation, and the `first` scalar
/// of a source.
fn order(description: &GraphDescription) -> Vec<String> {
    description
        .nodes
        .iter()
        .map(|node| match node.scalars.first() {
            Some((_, ScalarValue::I64(value))) => format!("{}({value})", node.implementation),
            Some(_) | None => node.implementation.clone(),
        })
        .collect()
}

fn edge(source_node: u32, target_node: u32, target_input: u32) -> Edge {
    Edge {
        source_node,
        target_node,
        target_input,
    }
}

// GRF-3, GRF-4; card, "Done when": `finish` orders producers before
// consumers, and each edge follows its nodes to their ranks.
#[test]
fn grf3_grf4_finish_puts_every_producer_before_its_consumers() {
    let registry = registry();
    let mut builder = Builder::new("diamond", &registry);
    let record = builder.node("record", &[]).unwrap();
    let difference = builder.node("difference", &[]).unwrap();
    let lhs = builder.node("source", &first(1)).unwrap();
    let rhs = builder.node("source", &first(2)).unwrap();
    builder.connect(difference, record, "in").unwrap();
    builder.connect(rhs, difference, "rhs").unwrap();
    builder.connect(lhs, difference, "lhs").unwrap();

    let description = builder.finish().unwrap();

    assert_eq!(description.label, "diamond");
    assert_eq!(
        order(&description),
        ["source(1)", "source(2)", "difference", "record"]
    );
    assert_eq!(
        description.edges,
        [edge(0, 2, 0), edge(1, 2, 1), edge(2, 3, 0)],
        "by target, then input position, with ranks for positions"
    );
}

// GRF-3: ranking is hgraph's (`build_ranked_graph`), first in, first out.
// Every node with no producer is ready at the start, in insertion order, and
// a node made ready joins the back of the queue — so the second source is
// ranked before the first source's consumer, which a queue ordered by
// insertion position (or by name, or last in, first out) would not do.
#[test]
fn grf3_finish_ranks_first_in_first_out_as_hgraph_does() {
    let registry = registry();
    let mut builder = Builder::new("two chains", &registry);
    let record = builder.node("record", &[]).unwrap();
    let one = builder.node("source", &first(1)).unwrap();
    let to_float = builder.node("to_float", &[]).unwrap();
    let two = builder.node("source", &first(2)).unwrap();
    builder.connect(one, to_float, "in").unwrap();
    builder.connect(two, record, "in").unwrap();

    let description = builder.finish().unwrap();

    assert_eq!(
        order(&description),
        ["source(1)", "source(2)", "to_float", "record"]
    );
    assert_eq!(description.edges, [edge(0, 2, 0), edge(1, 3, 0)]);
}

// GRF-3: a first-in-first-out ranking takes every node that has no producer
// before any node that has one, so a source added after a consumer is ranked
// before it, and insertion order is not kept.
#[test]
fn grf3_finish_ranks_every_source_before_what_waits_on_one() {
    let registry = registry();
    let mut builder = Builder::new("in order", &registry);
    let lhs = builder.node("source", &first(3)).unwrap();
    let rhs = builder.node("source", &first(1)).unwrap();
    let difference = builder.node("difference", &[]).unwrap();
    builder.node("source", &first(2)).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(lhs, difference, "lhs").unwrap();
    builder.connect(rhs, difference, "rhs").unwrap();
    builder.connect(difference, record, "in").unwrap();

    let description = builder.finish().unwrap();

    assert_eq!(
        order(&description),
        [
            "source(3)",
            "source(1)",
            "source(2)",
            "difference",
            "record"
        ]
    );
    assert_eq!(
        description.edges,
        [edge(0, 3, 0), edge(1, 3, 1), edge(3, 4, 0)]
    );
}

// GRF-4; card, "Done when": a cycle is rejected — two nodes feeding each
// other, a node feeding itself, and a cycle behind a node that can be ranked.
#[test]
fn grf4_a_cycle_is_rejected() {
    let registry = registry();

    let mut builder = Builder::new("pair", &registry);
    let a = builder.node("difference", &[]).unwrap();
    let b = builder.node("difference", &[]).unwrap();
    builder.connect(a, b, "lhs").unwrap();
    builder.connect(b, a, "lhs").unwrap();
    assert_eq!(builder.finish(), Err(BuildError::Cycle));

    let mut builder = Builder::new("self", &registry);
    let a = builder.node("difference", &[]).unwrap();
    builder.connect(a, a, "rhs").unwrap();
    assert_eq!(builder.finish(), Err(BuildError::Cycle));

    let mut builder = Builder::new("behind a source", &registry);
    let source = builder.node("source", &first(0)).unwrap();
    let a = builder.node("difference", &[]).unwrap();
    let b = builder.node("difference", &[]).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(source, a, "lhs").unwrap();
    builder.connect(a, b, "lhs").unwrap();
    builder.connect(b, a, "rhs").unwrap();
    builder.connect(b, record, "in").unwrap();
    assert_eq!(builder.finish(), Err(BuildError::Cycle));
}

// GRF-9; card, "Done when": an unknown implementation is rejected with the
// matching error, and nothing is added.
#[test]
fn grf9_an_unknown_implementation_is_rejected() {
    let registry = registry();
    let mut builder = Builder::new("unknown", &registry);
    assert_eq!(
        builder.node("nothing", &[]),
        Err(BuildError::UnknownImplementation("nothing".to_owned()))
    );
    let description = builder.finish().unwrap();
    assert!(description.nodes.is_empty());
}

// Card, "Done when": an unknown input is rejected with the matching error;
// so is an edge from a node that has no output.
#[test]
fn an_unknown_input_or_a_missing_output_is_rejected() {
    let registry = registry();
    let mut builder = Builder::new("names", &registry);
    let source = builder.node("source", &first(0)).unwrap();
    let record = builder.node("record", &[]).unwrap();
    let difference = builder.node("difference", &[]).unwrap();

    assert_eq!(
        builder.connect(source, difference, "middle"),
        Err(BuildError::UnknownInput {
            node: "difference".to_owned(),
            input: "middle".to_owned(),
        })
    );
    assert_eq!(
        builder.connect(record, difference, "lhs"),
        Err(BuildError::NoOutput {
            node: "record".to_owned()
        })
    );
    assert!(builder.finish().unwrap().edges.is_empty());
}

// GRF-7; card, "Done when": `connect` rejects a wrong type.
#[test]
fn grf7_connect_rejects_different_types() {
    let registry = registry();
    let mut builder = Builder::new("types", &registry);
    let source = builder.node("source", &first(0)).unwrap();
    let to_float = builder.node("to_float", &[]).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(source, to_float, "in").unwrap();

    assert_eq!(
        builder.connect(to_float, record, "in"),
        Err(BuildError::WrongType {
            node: "record".to_owned(),
            what: "in".to_owned(),
        })
    );
    assert_eq!(builder.finish().unwrap().edges, [edge(0, 2, 0)]);
}

// GRF-6; card, "Done when": an input bound twice is rejected. What is
// refused is the one (node, input): the node's other input, and the same
// input of another node, are still free.
#[test]
fn grf6_an_input_is_bound_at_most_once() {
    let registry = registry();
    let mut builder = Builder::new("twice", &registry);
    let one = builder.node("source", &first(1)).unwrap();
    let two = builder.node("source", &first(2)).unwrap();
    let difference = builder.node("difference", &[]).unwrap();
    let other = builder.node("difference", &[]).unwrap();
    builder.connect(one, difference, "lhs").unwrap();

    assert_eq!(
        builder.connect(two, difference, "lhs"),
        Err(BuildError::InputBoundTwice {
            node: "difference".to_owned(),
            input: "lhs".to_owned(),
        })
    );
    assert_eq!(builder.connect(one, difference, "rhs"), Ok(()));
    assert_eq!(builder.connect(two, other, "lhs"), Ok(()));
    assert_eq!(
        builder.finish().unwrap().edges,
        [edge(0, 2, 0), edge(0, 2, 1), edge(1, 3, 0)]
    );
}

// GRF-8; card, "Done when": an unknown scalar and a wrong type are each
// rejected with the matching error; so is a declared scalar left out or
// given twice. A node that conforms is added with its scalars.
#[test]
fn grf8_scalars_conform_to_the_node_type() {
    let registry = registry();
    let mut builder = Builder::new("scalars", &registry);
    let unknown = |scalar: &str| {
        Err(BuildError::UnknownScalar {
            node: "source".to_owned(),
            scalar: scalar.to_owned(),
        })
    };

    let extra = [
        ("first", ScalarValue::I64(1)),
        ("last", ScalarValue::I64(2)),
    ];
    assert_eq!(builder.node("source", &extra), unknown("last"));
    assert_eq!(builder.node("source", &[]), unknown("first"));
    let twice = [
        ("first", ScalarValue::I64(1)),
        ("first", ScalarValue::I64(2)),
    ];
    assert_eq!(builder.node("source", &twice), unknown("first"));
    assert_eq!(
        builder.node("source", &[("first", ScalarValue::F64(1.0))]),
        Err(BuildError::WrongType {
            node: "source".to_owned(),
            what: "first".to_owned(),
        })
    );
    assert_eq!(
        builder.node("record", &[("first", ScalarValue::I64(1))]),
        Err(BuildError::UnknownScalar {
            node: "record".to_owned(),
            scalar: "first".to_owned(),
        })
    );

    builder.node("source", &first(7)).unwrap();
    let description = builder.finish().unwrap();
    assert_eq!(description.nodes.len(), 1, "a refused node is not added");
    assert_eq!(description.nodes[0].label, "source");
    assert_eq!(
        description.nodes[0].scalars,
        [("first".to_owned(), ScalarValue::I64(7))]
    );
}

// GRF-9: a name is taken once. The implementation registered first stays.
#[test]
fn grf9_a_name_is_registered_once_and_the_first_stays() {
    let mut registry = registry();
    assert_eq!(
        registry.register::<Impostor>(),
        Err(BuildError::DuplicateImplementation("source"))
    );
    assert_eq!(registry.node_type("source"), Some(&Source::node_type()));
    assert_eq!(
        registry.register::<Source>(),
        Err(BuildError::DuplicateImplementation("source"))
    );
    assert_eq!(registry.node_type("record"), Some(&Record::node_type()));
}

// GRF-1 is held by the types: every field of a description is a `String`, a
// number or a `ScalarValue`, so it can hold nothing live and this can only
// show that a copy of one compares equal.
#[test]
fn grf1_a_description_is_plain_data() {
    let registry = registry();
    let mut builder = Builder::new("copy", &registry);
    let source = builder.node("source", &first(1)).unwrap();
    let record = builder.node("record", &[]).unwrap();
    builder.connect(source, record, "in").unwrap();
    let description = builder.finish().unwrap();

    let copy = description.clone();
    assert_eq!(copy, description);
    assert_ne!(
        copy,
        GraphDescription {
            label: "other".to_owned(),
            ..description
        }
    );
}

// GRF-3: one graph gives one description, whatever order it was connected
// in. The edges of a node are ranked and emitted in input order, as hgraph
// emits them, so nothing of the connect order survives.
#[test]
fn grf3_two_builds_of_one_graph_compare_equal() {
    let registry = registry();
    let build = |connect_in_reverse: bool| {
        let mut builder = Builder::new("two ways", &registry);
        let one = builder.node("source", &first(1)).unwrap();
        let two = builder.node("source", &first(2)).unwrap();
        let record = builder.node("record", &[]).unwrap();
        let to_float = builder.node("to_float", &[]).unwrap();
        let difference = builder.node("difference", &[]).unwrap();
        // Two consumers of one source, told apart by their implementation:
        // the order they are reached in is the order they were added in, not
        // the order they were connected in.
        let mut connects: Vec<(NodeRef, NodeRef, &str)> = vec![
            (one, record, "in"),
            (one, to_float, "in"),
            (one, difference, "lhs"),
            (two, difference, "rhs"),
        ];
        if connect_in_reverse {
            connects.reverse();
        }
        for (source, target, input) in connects {
            builder.connect(source, target, input).unwrap();
        }
        builder.finish().unwrap()
    };

    let description = build(false);
    assert_eq!(description, build(true));
    assert_eq!(
        order(&description),
        ["source(1)", "source(2)", "record", "to_float", "difference"]
    );
    assert_eq!(
        description.edges,
        [edge(0, 2, 0), edge(0, 3, 0), edge(0, 4, 0), edge(1, 4, 1)]
    );
}

// A node asks for its ports by name, so two inputs of one name would leave
// one of them unreachable.
#[test]
fn register_refuses_two_inputs_of_one_name() {
    let mut registry = registry();
    assert_eq!(
        registry.register::<TwoOfOneName>(),
        Err(BuildError::InvalidNodeType {
            node: "two_of_one_name",
            what: "input in twice".to_owned(),
        })
    );
    assert_eq!(registry.node_type("two_of_one_name"), None);
}

// NOD-2, NOD-4: `active_inputs` and `valid_inputs` are positions in
// `inputs`, and instantiation resolves them to store ids. One position past
// the last input is refused here, where the node type is; it used to pass
// registration and then panic, or silently make nothing active.
#[test]
fn nod2_nod4_register_refuses_a_position_past_the_last_input() {
    let mut registry = registry();
    assert_eq!(
        registry.register::<ActivePastTheEnd>(),
        Err(BuildError::InvalidNodeType {
            node: "active_past_the_end",
            what: "active input 1".to_owned(),
        })
    );
    assert_eq!(
        registry.register::<ValidPastTheEnd>(),
        Err(BuildError::InvalidNodeType {
            node: "valid_past_the_end",
            what: "valid input 1".to_owned(),
        })
    );
    assert_eq!(registry.node_type("active_past_the_end"), None);
    assert_eq!(registry.node_type("valid_past_the_end"), None);
}
