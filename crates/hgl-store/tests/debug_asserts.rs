//! What release builds trust, debug builds assert (card, "Speed"). Each test
//! breaks one such trust and expects the assertion that names it. In a release
//! build these calls are the caller's bug and are not looked for, so the file
//! is empty there.
#![cfg(debug_assertions)]

use hgl_store::{In, Out, Store, Wake};
use hgl_types::{EngineTime, NodeId};

const PRODUCER: NodeId = NodeId(0);
const CONSUMER: NodeId = NodeId(1);

/// A wake is of no interest here.
struct Nobody;

impl Wake for Nobody {
    fn wake(&mut self, _node: NodeId) {}
}

fn at(micros: i64) -> EngineTime {
    EngineTime::from_micros(micros)
}

/// A `TS<i64>` output of `PRODUCER` with an input of `CONSUMER` bound to it.
fn bound_pair() -> (Store, Out<i64>, In<i64>) {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    assert_eq!(store.bind(input.id(), output.id()), Ok(()));
    (store, output, input)
}

// TS-2: an input that is not valid has no value.
#[test]
#[should_panic(expected = "TS-2")]
fn ts2_get_on_an_input_that_is_not_valid_asserts() {
    let (store, _output, input) = bound_pair();
    let _nil = store.get(input);
}

// TS-3: last modified time never decreases.
#[test]
#[should_panic(expected = "TS-3")]
fn ts3_a_tick_earlier_than_the_last_asserts() {
    let (mut store, output, _input) = bound_pair();
    store.set(output, 1, at(10), PRODUCER, &mut Nobody);
    store.set(output, 2, at(9), PRODUCER, &mut Nobody);
}

// TS-21: only an output's own node changes it.
#[test]
#[should_panic(expected = "TS-21")]
fn ts21_set_by_a_node_that_does_not_own_the_output_asserts() {
    let (mut store, output, _input) = bound_pair();
    store.set(output, 7, at(10), CONSUMER, &mut Nobody);
}

// TS-1: `NEVER` means "has not ticked", so nothing may tick at it.
#[test]
#[should_panic(expected = "NEVER is not an evaluation time")]
fn ts1_a_tick_at_never_asserts() {
    let (mut store, output, _input) = bound_pair();
    store.set(output, 7, EngineTime::NEVER, PRODUCER, &mut Nobody);
}

// TS-1: asked about `NEVER`, an unbound input would read modified.
#[test]
#[should_panic(expected = "NEVER is not an evaluation time")]
fn ts1_modified_asked_about_never_asserts() {
    let mut store = Store::new();
    let input = store.add_input::<i64>(CONSUMER, true);
    let _modified = store.modified(input, EngineTime::NEVER);
}

// TS-1: and so would an output that has not ticked, on the erased path.
#[test]
#[should_panic(expected = "NEVER is not an evaluation time")]
fn ts1_output_modified_asked_about_never_asserts() {
    let (store, output, _input) = bound_pair();
    let _modified = store.output_modified(output.id(), EngineTime::NEVER);
}

/// A store whose output 0 and input 0 are `i64`, bound and ticked, so that
/// nothing but the handle is wrong; and `f64` handles 0 from another store.
fn a_store_and_foreign_handles() -> (Store, Out<f64>, In<f64>) {
    let (mut ours, output, _input) = bound_pair();
    ours.set(output, 1, at(10), PRODUCER, &mut Nobody);
    let mut theirs = Store::new();
    let foreign_out = theirs.add_output::<f64>(PRODUCER);
    let foreign_in = theirs.add_input::<f64>(CONSUMER, true);
    (ours, foreign_out, foreign_in)
}

// Card, "Speed": the handle's type matches its column, for `get`.
#[test]
#[should_panic(expected = "foreign handle")]
fn get_through_a_handle_of_another_store_asserts() {
    let (ours, _foreign_out, foreign_in) = a_store_and_foreign_handles();
    let _value = ours.get(foreign_in);
}

// Card, "Speed": the handle's type matches its column, for `set`.
#[test]
#[should_panic(expected = "foreign handle")]
fn set_through_a_handle_of_another_store_asserts() {
    let (mut ours, foreign_out, _foreign_in) = a_store_and_foreign_handles();
    ours.set(foreign_out, 1.0, at(11), PRODUCER, &mut Nobody);
}

// Card, "Speed": the handle's type matches its column, for `output_value`.
#[test]
#[should_panic(expected = "foreign handle")]
fn output_value_through_a_handle_of_another_store_asserts() {
    let (ours, foreign_out, _foreign_in) = a_store_and_foreign_handles();
    let _value = ours.output_value(foreign_out);
}
