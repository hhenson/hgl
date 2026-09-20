//! The case tables: what a node does, written as ticks in and ticks out.
//!
//! A case is hgraph's `eval_node` written as data. Each input is a sequence
//! with one element per engine cycle, `None` meaning *no tick*; the expected
//! output is a sequence of the same kind. Cycle *i* runs at the start time
//! plus *i* smallest steps, which is hgraph's alignment, so its cases
//! transcribe without change.
//!
//! Every case says where it came from: the hgraph test it was transcribed
//! from, or the rule of the runtime specification it was written from.

use hgl_types::ScalarValue;

/// One element per engine cycle; `None` is *no tick*.
pub type Ticks = &'static [Option<ScalarValue>];

/// The slice of the prototype that has to make a case pass
/// (`docs/explorations/0008-prototype-outline.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slice {
    /// One value moving: TS, the cycle, lifecycle, admission.
    P1,
    /// Time of its own: the node scheduler, passive inputs.
    P2,
}

/// One node, driven for a few cycles, and what it must produce.
#[derive(Debug, Clone, Copy)]
pub struct Case {
    /// What the case shows, as a sentence.
    pub name: &'static str,
    /// The hgraph test, or the specification rule, this was taken from.
    pub source: &'static str,
    /// The node under test, by the name its implementation is registered under.
    pub node: &'static str,
    /// The node's scalars, by name.
    pub scalars: &'static [(&'static str, ScalarValue)],
    /// The node's inputs, by name.
    pub inputs: &'static [(&'static str, Ticks)],
    /// What its output must show, cycle by cycle.
    pub expected: Ticks,
    /// Stop after this many cycles, as an end time would. `None` runs until
    /// nothing is scheduled.
    pub cycles: Option<usize>,
    /// The slice that has to make this pass.
    pub slice: Slice,
}

const NO: Option<ScalarValue> = None;

#[expect(
    clippy::unnecessary_wraps,
    reason = "every cell of a tick sequence is an Option, so the tables read evenly"
)]
const fn i(value: i64) -> Option<ScalarValue> {
    Some(ScalarValue::I64(value))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "every cell of a tick sequence is an Option, so the tables read evenly"
)]
const fn b(value: bool) -> Option<ScalarValue> {
    Some(ScalarValue::Bool(value))
}

const HGRAPH: &str = "hgraph tests/cpp/test_eval_node.cpp";

/// Every case, in the order the prototype is expected to make them pass.
pub const ALL: &[Case] = &[
    Case {
        name: "a compute node maps each input tick, and a skipped cycle stays skipped",
        source: HGRAPH,
        node: "add_one",
        scalars: &[],
        inputs: &[("in", &[i(1), NO, i(3)])],
        expected: &[i(2), NO, i(4)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "a source with no inputs runs once, because it asked to at start",
        source: HGRAPH,
        node: "constant_41",
        scalars: &[],
        inputs: &[],
        expected: &[i(41)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "a source reads its scalar",
        source: HGRAPH,
        node: "configured_source",
        scalars: &[("value", ScalarValue::I64(12))],
        inputs: &[],
        expected: &[i(12)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "node state persists from cycle to cycle",
        source: HGRAPH,
        node: "running_sum",
        scalars: &[],
        inputs: &[("in", &[i(1), i(2), i(3), i(4)])],
        expected: &[i(1), i(3), i(6), i(10)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "no input ticks, no output ticks",
        source: HGRAPH,
        node: "add_one",
        scalars: &[],
        inputs: &[("in", &[NO, NO])],
        expected: &[NO, NO],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "a scalar is passed to the node under test",
        source: HGRAPH,
        node: "shift",
        scalars: &[("delta", ScalarValue::I64(5))],
        inputs: &[("in", &[i(1), i(2), i(3)])],
        expected: &[i(6), i(7), i(8)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "an input that does not tick still shows its last value",
        source: HGRAPH,
        node: "sum",
        scalars: &[],
        inputs: &[("lhs", &[i(1), NO, i(3)]), ("rhs", &[i(10), i(20), i(30)])],
        expected: &[i(11), i(21), i(33)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "the start time is inclusive and the end time is exclusive",
        source: HGRAPH,
        node: "add_one",
        scalars: &[],
        inputs: &[("in", &[i(3), NO, i(8)])],
        expected: &[i(4), NO],
        cycles: Some(2),
        slice: Slice::P1,
    },
    Case {
        name: "a node is not evaluated until every input it requires is valid",
        source: "rule NOD-2",
        node: "sum",
        scalars: &[],
        inputs: &[("lhs", &[i(1), NO, NO]), ("rhs", &[NO, NO, i(10)])],
        expected: &[NO, NO, i(11)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "a node that nothing notified is not evaluated",
        source: "rules NOD-4, GRF-11",
        node: "running_sum",
        scalars: &[],
        inputs: &[("in", &[i(5), NO, NO, i(1)])],
        expected: &[i(5), NO, NO, i(6)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "writing the value an output already holds is still a tick",
        source: "Time-series types, 'A tick'",
        node: "add_one",
        scalars: &[],
        inputs: &[("in", &[i(1), i(1)])],
        expected: &[i(2), i(2)],
        cycles: None,
        slice: Slice::P1,
    },
    Case {
        name: "a node may tick later than its input, by asking its scheduler",
        source: HGRAPH,
        node: "echo_once",
        scalars: &[],
        inputs: &[("in", &[i(5)])],
        expected: &[i(5), i(105)],
        cycles: None,
        slice: Slice::P2,
    },
    Case {
        name: "a passive input is read but does not wake the node",
        source: "rules GRF-17, TS-8",
        node: "sample",
        scalars: &[],
        inputs: &[
            ("trigger", &[NO, b(true), NO, b(true)]),
            ("value", &[i(1), i(2), i(3), NO]),
        ],
        expected: &[NO, i(2), NO, i(3)],
        cycles: None,
        slice: Slice::P2,
    },
];
