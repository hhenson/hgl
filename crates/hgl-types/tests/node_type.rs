//! The node type and the scalar vocabulary (specification: Graph, Part 1,
//! "Node type").

use hgl_types::{NodeId, NodeKind, NodeType, ScalarType, ScalarValue, TsType};

const TS_F64: TsType = TsType::Ts(ScalarType::F64);

fn node_type(inputs: Vec<(&'static str, TsType)>, output: Option<TsType>) -> NodeType {
    NodeType {
        name: "under_test",
        inputs,
        output,
        scalars: Vec::new(),
        active_inputs: None,
        valid_inputs: None,
        uses_scheduler: false,
        schedule_on_start: false,
        uses_global_state: false,
        global_entries: Vec::new(),
        child_graphs: 0,
    }
}

// Card, NodeType::kind: no inputs and an output is a pull source.
#[test]
fn no_inputs_and_an_output_is_a_pull_source() {
    let signature = node_type(Vec::new(), Some(TS_F64));
    assert_eq!(signature.kind(), NodeKind::PullSource);
}

// Card, NodeType::kind: inputs and an output, compute.
#[test]
fn inputs_and_an_output_is_compute() {
    let signature = node_type(vec![("lhs", TS_F64), ("rhs", TS_F64)], Some(TS_F64));
    assert_eq!(signature.kind(), NodeKind::Compute);
}

// Card, NodeType::kind: inputs and no output, a sink.
#[test]
fn inputs_and_no_output_is_a_sink() {
    let signature = node_type(vec![("ts", TS_F64)], None);
    assert_eq!(signature.kind(), NodeKind::Sink);
}

// Card, NodeType::kind: neither, compute (as hgraph). The oracle: hgraph's
// static_node.h, node_kind().
#[test]
fn neither_inputs_nor_an_output_is_compute_as_in_hgraph() {
    let signature = node_type(Vec::new(), None);
    assert_eq!(signature.kind(), NodeKind::Compute);
}

/// Sets every field that `kind` must not read.
fn with_the_rest_set(mut signature: NodeType) -> NodeType {
    signature.scalars = vec![("period", ScalarType::I64)];
    signature.active_inputs = Some(Vec::new());
    signature.valid_inputs = Some(Vec::new());
    signature.uses_scheduler = true;
    signature.schedule_on_start = true;
    signature
}

// Card, NodeType::kind: "from the signature" — a node that schedules itself
// is still a sink, or still compute, if its inputs and output say so.
#[test]
fn kind_reads_only_inputs_and_output() {
    let sink = with_the_rest_set(node_type(vec![("ts", TS_F64)], None));
    assert_eq!(sink.kind(), NodeKind::Sink);
    let neither = with_the_rest_set(node_type(Vec::new(), None));
    assert_eq!(neither.kind(), NodeKind::Compute);
}

// Card, ScalarValue::scalar_type: each value reports its own type.
#[test]
fn a_scalar_value_knows_its_type() {
    assert_eq!(ScalarValue::Bool(true).scalar_type(), ScalarType::Bool);
    assert_eq!(ScalarValue::I64(-3).scalar_type(), ScalarType::I64);
    assert_eq!(ScalarValue::F64(0.5).scalar_type(), ScalarType::F64);
}

// Card, "Speed": NodeId and ScalarType are Copy and one word.
#[test]
fn node_id_and_scalar_type_are_copy_and_one_word() {
    let id = NodeId(7);
    let copy = id;
    assert_eq!(id, copy);
    let scalar = ScalarType::I64;
    let same = scalar;
    assert_eq!(scalar, same);
    assert_eq!(size_of::<NodeId>(), size_of::<u32>());
    assert_eq!(size_of::<ScalarType>(), 1);
}

// Card, NodeId: a position in rank order, so ids order as their positions.
#[test]
fn node_ids_order_as_their_positions() {
    assert!(NodeId(1) < NodeId(2));
}

#[test]
fn a_template_owner_is_nested_regardless_of_its_signature() {
    let owner = NodeType {
        child_graphs: 1,
        ..NodeType::default()
    };
    assert_eq!(owner.kind(), NodeKind::Nested);
}
