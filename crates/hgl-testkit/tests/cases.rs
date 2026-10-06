//! The case tables themselves, and the harness run over them.

use hgl_describe::{BuildError, Registry};
use hgl_testkit::cases::{ALL, Case, Slice, Ticks};
use hgl_testkit::{Failure, Record, Replay, run};
use hgl_types::ScalarValue;

/// Every node a case can name, and the harness's own two.
fn registry() -> Result<Registry, BuildError> {
    let mut registry = Registry::new();
    hgl_testkit::proto_nodes::register_all(&mut registry)?;
    registry.register::<Replay>()?;
    registry.register::<Record>()?;
    Ok(registry)
}

/// The first case: `add_one` over `[1, -, 3]`, expecting `[2, -, 4]`.
fn add_one() -> Case {
    ALL[0]
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "every cell of a tick sequence is an Option, so the tables read evenly"
)]
const fn i(value: i64) -> Option<ScalarValue> {
    Some(ScalarValue::I64(value))
}

const NO: Option<ScalarValue> = None;

/// What `add_one` over `[1, -, 3]` really produces, and the three ways a case
/// can ask for something else.
const RIGHT: Ticks = &[i(2), NO, i(4)];
const WRONG_IN_THE_LAST_CYCLE: Ticks = &[i(2), NO, i(5)];
const SHORT_OF_A_TICK: Ticks = &[i(2)];
const A_TICK_TOO_MANY: Ticks = &[i(2), NO];

/// `add_one`'s input, and one it never ticks for.
const ONE_AND_THREE: &[(&str, Ticks)] = &[("in", &[i(1), NO, i(3)])];
const NOTHING: &[(&str, Ticks)] = &[("in", &[NO, NO])];
const NOT_AN_INPUT: &[(&str, Ticks)] = &[("nope", &[i(1)])];

#[test]
fn every_case_says_where_it_came_from() {
    for case in ALL {
        assert!(!case.name.is_empty());
        assert!(!case.node.is_empty(), "{}", case.name);
        assert!(
            case.source.starts_with("hgraph ")
                || case.source.starts_with("rule")
                || case.source.starts_with("Time-series types"),
            "{}: '{}' is neither an hgraph test nor a rule of the specification",
            case.name,
            case.source
        );
    }
}

#[test]
fn case_names_are_unique() {
    for (index, case) in ALL.iter().enumerate() {
        let repeated = ALL
            .iter()
            .skip(index + 1)
            .any(|other| other.name == case.name);
        assert!(!repeated, "'{}' appears twice", case.name);
    }
}

#[test]
fn a_case_cut_short_expects_no_more_cycles_than_it_runs() {
    for case in ALL {
        if let Some(cycles) = case.cycles {
            assert!(case.expected.len() <= cycles, "{}", case.name);
        }
    }
}

#[test]
fn first_slice_cases_pass() {
    let registry = registry().unwrap();
    for case in ALL.iter().filter(|case| case.slice == Slice::P1) {
        assert_eq!(run(case, &registry), Ok(()), "{}", case.name);
    }
}

#[test]
#[ignore = "P2: needs the node scheduler and passive inputs"]
fn second_slice_cases_pass() {
    let registry = registry().unwrap();
    for case in ALL.iter().filter(|case| case.slice == Slice::P2) {
        assert_eq!(run(case, &registry), Ok(()), "{}", case.name);
    }
}

/// ENG-3: the start time is inclusive, so a run of one cycle runs it, and the
/// end time is exclusive, so it runs no other.
#[test]
fn a_run_of_one_cycle_runs_at_the_start_time_and_stops() {
    let case = Case {
        cycles: Some(1),
        expected: SHORT_OF_A_TICK,
        ..add_one()
    };
    assert_eq!(run(&case, &registry().unwrap()), Ok(()));
}

#[test]
fn a_node_the_registry_does_not_hold_is_a_failure() {
    let case = Case {
        node: "no_such_node",
        ..add_one()
    };
    let failed = run(&case, &registry().unwrap());
    assert_eq!(failed, Err(Failure::UnknownNode("no_such_node")));
}

#[test]
fn a_node_with_no_output_is_a_failure() {
    let case = Case {
        node: "checksum",
        ..add_one()
    };
    assert_eq!(
        run(&case, &registry().unwrap()),
        Err(Failure::NoOutput("checksum"))
    );
}

#[test]
fn without_the_harnesss_own_nodes_there_is_no_harness() {
    let mut registry = Registry::new();
    hgl_testkit::proto_nodes::register_all(&mut registry).unwrap();
    let missing = BuildError::UnknownImplementation("testkit.replay".to_owned());
    assert_eq!(run(&add_one(), &registry), Err(Failure::Build(missing)));
}

#[test]
fn a_wrong_expectation_is_reported_as_the_first_cycle_that_differs() {
    let case = Case {
        expected: WRONG_IN_THE_LAST_CYCLE,
        ..add_one()
    };
    let mismatch = Failure::Mismatch {
        cycle: 2,
        expected: i(5),
        actual: i(4),
    };
    assert_eq!(run(&case, &registry().unwrap()), Err(mismatch));
}

#[test]
fn a_tick_the_case_does_not_expect_is_a_difference() {
    let case = Case {
        expected: SHORT_OF_A_TICK,
        ..add_one()
    };
    let mismatch = Failure::Mismatch {
        cycle: 2,
        expected: NO,
        actual: i(4),
    };
    assert_eq!(run(&case, &registry().unwrap()), Err(mismatch));
}

#[test]
fn a_node_that_never_ticks_does_not_pass_a_case_that_expects_one() {
    let case = Case {
        inputs: NOTHING,
        expected: A_TICK_TOO_MANY,
        ..add_one()
    };
    let mismatch = Failure::Mismatch {
        cycle: 0,
        expected: i(2),
        actual: NO,
    };
    assert_eq!(run(&case, &registry().unwrap()), Err(mismatch));
}

#[test]
fn a_case_the_builder_refuses_is_a_failure() {
    let case = Case {
        inputs: NOT_AN_INPUT,
        ..add_one()
    };
    let refused = BuildError::UnknownInput {
        node: "add_one".to_owned(),
        input: "nope".to_owned(),
    };
    assert_eq!(
        run(&case, &registry().unwrap()),
        Err(Failure::Build(refused))
    );
}

#[test]
fn the_case_the_others_are_cut_from_is_the_one_in_the_table() {
    let case = add_one();
    assert_eq!(
        (case.node, case.inputs, case.expected),
        ("add_one", ONE_AND_THREE, RIGHT)
    );
}
