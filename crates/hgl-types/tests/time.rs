//! Engine time: the four named times and the smallest step (specification:
//! Execution engine, rule ENG-16), and the card's `checked_add`.

use hgl_types::{EngineDelta, EngineTime};

const NEVER: EngineTime = EngineTime::NEVER;
const MIN_START: EngineTime = EngineTime::MIN_START;
const MAX_END: EngineTime = EngineTime::MAX_END;
const FOREVER: EngineTime = EngineTime::FOREVER;
const STEP: EngineDelta = EngineDelta::STEP;

fn back(delta: EngineDelta) -> EngineDelta {
    EngineDelta::from_micros(-delta.micros())
}

/// Counted year by year, so it does not repeat the constant under test.
fn days_from_the_epoch_to_2300() -> i64 {
    let mut days = 0;
    for year in 1970..2300 {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        days += if leap { 366 } else { 365 };
    }
    days
}

// ENG-16: never is before earliest start, before latest end, before forever.
#[test]
fn the_named_times_are_in_order() {
    assert!(NEVER < MIN_START);
    assert!(MIN_START < MAX_END);
    assert!(MAX_END < FOREVER);
}

// ENG-16: the smallest step is one microsecond.
#[test]
fn the_smallest_step_is_one_microsecond() {
    assert_eq!(STEP.micros(), 1);
}

// ENG-16: earliest start is never plus one smallest step.
#[test]
fn min_start_is_never_plus_one_step() {
    assert_eq!(MIN_START.micros(), NEVER.micros() + STEP.micros());
    assert_eq!(NEVER.checked_add(STEP), Some(MIN_START));
}

// ENG-16: latest end is forever minus one smallest step.
#[test]
fn max_end_is_forever_minus_one_step() {
    assert_eq!(MAX_END.micros(), FOREVER.micros() - STEP.micros());
    assert_eq!(MAX_END.checked_add(STEP), Some(FOREVER));
}

// Card, Rules: NEVER is the epoch and FOREVER is 2300-01-01, as in hgraph.
#[test]
fn never_and_forever_are_the_instants_hgraph_uses() {
    assert_eq!(NEVER.micros(), 0);
    let micros_per_day = 86_400 * 1_000_000;
    assert_eq!(
        FOREVER.micros(),
        days_from_the_epoch_to_2300() * micros_per_day
    );
}

// ENG-16: no arithmetic on a time gives a result after forever.
#[test]
fn checked_add_refuses_to_pass_forever() {
    assert_eq!(FOREVER.checked_add(STEP), None);
    assert_eq!(MAX_END.checked_add(EngineDelta::from_micros(2)), None);
}

// ENG-16: no arithmetic on a time gives a result before never.
#[test]
fn checked_add_refuses_to_go_before_never() {
    assert_eq!(NEVER.checked_add(back(STEP)), None);
    assert_eq!(MIN_START.checked_add(EngineDelta::from_micros(-2)), None);
    assert_eq!(NEVER.checked_add(EngineDelta::from_micros(i64::MIN)), None);
}

// Card, checked_add: `None` if the addition overflows. The second sum wraps
// to 0, which is NEVER, so only the overflow itself can refuse it.
#[test]
fn checked_add_refuses_a_sum_that_overflows() {
    assert_eq!(
        FOREVER.checked_add(EngineDelta::from_micros(i64::MAX)),
        None
    );
    let lowest = EngineTime::from_micros(i64::MIN);
    assert_eq!(lowest.checked_add(EngineDelta::from_micros(i64::MIN)), None);
}

// Card, checked_add: judged on the result alone, so a `self` outside the
// range may come back inside it.
#[test]
fn checked_add_judges_the_result_alone() {
    let before_never = EngineTime::from_micros(-10);
    let sum = before_never.checked_add(EngineDelta::from_micros(20));
    assert_eq!(sum, Some(EngineTime::from_micros(10)));
}

// ENG-16: the range is never to forever, both ends included.
#[test]
fn checked_add_reaches_both_ends_of_the_range() {
    let whole_range = EngineDelta::from_micros(FOREVER.micros() - NEVER.micros());
    assert_eq!(NEVER.checked_add(whole_range), Some(FOREVER));
    assert_eq!(FOREVER.checked_add(back(whole_range)), Some(NEVER));
    assert_eq!(
        FOREVER.checked_add(EngineDelta::from_micros(0)),
        Some(FOREVER)
    );
}

// Card: an ordinary sum inside the range is the sum of the microseconds.
#[test]
fn checked_add_adds_microseconds() {
    let start = EngineTime::from_micros(1_000);
    let later = start.checked_add(EngineDelta::from_micros(250));
    assert_eq!(later, Some(EngineTime::from_micros(1_250)));
    let earlier = start.checked_add(EngineDelta::from_micros(-250));
    assert_eq!(earlier, Some(EngineTime::from_micros(750)));
}

// Card, surface: from_micros and micros are each other's inverse.
#[test]
fn micros_round_trip() {
    assert_eq!(EngineTime::from_micros(42).micros(), 42);
    assert_eq!(EngineDelta::from_micros(-42).micros(), -42);
}

// Card, "Speed": comparing two times is comparing their microseconds.
#[test]
fn times_and_deltas_order_as_their_microseconds() {
    assert!(EngineTime::from_micros(1) < EngineTime::from_micros(2));
    assert!(EngineDelta::from_micros(-1) < EngineDelta::from_micros(0));
}

// Card, "Speed": time and delta are Copy and one word. Using a value after
// assigning it elsewhere compiles only if its type is Copy.
#[test]
fn time_and_delta_are_copy_and_one_word() {
    let time = MIN_START;
    let time_copy = time;
    assert_eq!(time, time_copy);
    let delta = STEP;
    let delta_copy = delta;
    assert_eq!(delta, delta_copy);
    assert_eq!(size_of::<EngineTime>(), size_of::<i64>());
    assert_eq!(size_of::<EngineDelta>(), size_of::<i64>());
}
