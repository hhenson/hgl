//! ADR 0016 storage, ownership and construction-binding acceptance.
use hgl_kernel::NodeError;
use hgl_std_native::eval_buffers::{
    BufferBinding, BufferRequirement, BufferRole, BufferScalar, Capture, ReplayInput,
    validate_bindings,
};
use hgl_types::{Date, EngineDelta, EngineTime, ScalarType, Time};

type Result<T> = std::result::Result<T, Box<NodeError>>;

fn time(micros: i64) -> EngineTime {
    EngineTime::from_micros(micros)
}
fn message<T>(result: Result<T>) -> String {
    result.err().map_or_else(String::new, |error| error.message)
}

fn scalar_round_trip<T: BufferScalar>(value: T) -> Result<()> {
    let input = ReplayInput::new(
        vec![Some(value.copy_delta()?), None, Some(value.copy_delta()?)],
        EngineTime::MIN_START,
    )?;
    let mut capture = Capture::new();
    assert_eq!(input.length(), 3);
    assert_eq!(input.get(0)?, Some(value.copy_delta()?));
    assert_eq!(input.get(1)?, None);
    capture.begin()?;
    let first = input
        .get(0)?
        .ok_or_else(|| NodeError::new("expected present first slot"))?;
    let last = input
        .get(2)?
        .ok_or_else(|| NodeError::new("expected present last slot"))?;
    capture.append(time(1), &first, time(1))?;
    capture.append(time(3), &last, time(3))?;
    assert_eq!(
        capture.take_ticks()?,
        vec![(time(1), value.copy_delta()?), (time(3), value)]
    );
    Ok(())
}

#[test]
fn eight_scalar_types_keep_present_equal_ticks() -> Result<()> {
    scalar_round_trip(false)?;
    scalar_round_trip(0_i64)?;
    scalar_round_trip(0.0_f64)?;
    scalar_round_trip(String::new())?;
    scalar_round_trip(Date(0))?;
    scalar_round_trip(Time(0))?;
    scalar_round_trip(EngineTime::from_micros(0))?;
    scalar_round_trip(EngineDelta::from_micros(0))
}

#[test]
fn owned_temporal_scalars_keep_exact_identity_and_equal_ticks() -> Result<()> {
    scalar_round_trip(hgl_types::ZonedTime::from_validated_parts(
        Time(34_200_123_456),
        hgl_types::ZoneId::from_validated_name("US/Eastern".into()),
    ))?;
    scalar_round_trip(hgl_types::CivilDateTime::from_micros(123_456))?;
    scalar_round_trip(hgl_types::ZoneId::from_validated_name("US/Eastern".into()))?;
    scalar_round_trip(hgl_types::ZonedDateTime::from_validated_parts(
        time(0),
        hgl_types::ZoneId::from_validated_name("Etc/UTC".into()),
        0,
    ))
}

#[test]
fn input_bounds_and_absence_never_create_default_values() -> Result<()> {
    let input = ReplayInput::new(vec![Some(0_i64), None], time(1))?;
    for index in [-1, 2, i64::MAX] {
        assert_eq!(
            message(input.get(index)),
            "replay_input: index out of range"
        );
    }
    assert_eq!(input.get(1)?, None);
    assert_eq!(input.get(1)?, None);
    assert_eq!(input.get(0)?, Some(0));
    assert_eq!(input.length(), 2);
    Ok(())
}

#[test]
fn empty_slots_and_unbegun_capture_are_distinct() -> Result<()> {
    let empty = ReplayInput::<i64>::new(Vec::new(), time(1))?;
    let silent = ReplayInput::<i64>::new(vec![None, None], time(1))?;
    assert_eq!(empty.length(), 0);
    assert_eq!(silent.length(), 2);
    assert_eq!(message(empty.get(0)), "replay_input: index out of range");
    assert_eq!(silent.get(0)?, None);
    assert_eq!(silent.get(1)?, None);
    let mut capture = Capture::<i64>::new();
    assert_eq!(message(capture.take_ticks()), "capture: not begun");
    capture.begin()?;
    assert!(capture.take_ticks()?.is_empty());
    assert_eq!(message(capture.begin()), "capture: already begun");
    Ok(())
}

#[test]
fn append_validates_in_order_and_preserves_earlier_captures() -> Result<()> {
    let mut capture = Capture::new();
    assert_eq!(
        message(capture.append(time(3), &7_i64, time(4))),
        "capture: not begun"
    );
    capture.begin()?;
    capture.append(time(3), &7, time(3))?;
    assert_eq!(message(capture.begin()), "capture: already begun");
    assert_eq!(
        message(capture.append(time(2), &9, time(4))),
        "capture: timestamp is not evaluation time"
    );
    assert_eq!(
        message(capture.append(time(2), &9, time(2))),
        "capture: timestamp did not advance"
    );
    assert_eq!(
        message(capture.append(time(3), &9, time(3))),
        "capture: timestamp did not advance"
    );
    capture.append(time(4), &7, time(4))?;
    assert_eq!(capture.take_ticks()?, vec![(time(3), 7), (time(4), 7)]);
    Ok(())
}

#[test]
fn text_copies_survive_input_changes_drop_and_another_run() -> Result<()> {
    let input = ReplayInput::new(vec![Some("first".to_owned())], time(1))?;
    let mut value = input.get(0)?.expect("present text slot");
    let repeated = input.get(0)?.expect("repeated present text slot");
    let mut capture = Capture::new();
    capture.begin()?;
    capture.append(time(1), &value, time(1))?;
    value.clear();
    value.push_str("second");
    capture.append(time(2), &value, time(2))?;
    assert_eq!(input.get(0)?, Some("first".to_owned()));
    assert_eq!(repeated, "first");
    drop(input);
    drop(value);
    assert_eq!(repeated, "first");
    let ticks = capture.take_ticks()?;
    drop(capture);
    let mut fresh = Capture::<String>::new();
    fresh.begin()?;
    assert!(fresh.take_ticks()?.is_empty());
    assert_eq!(
        ticks,
        vec![
            (time(1), "first".to_owned()),
            (time(2), "second".to_owned())
        ]
    );
    Ok(())
}

#[test]
fn final_dense_time_must_be_before_the_exclusive_latest_end() -> Result<()> {
    let latest_tick = time(EngineTime::MAX_END.micros() - 1);
    assert_eq!(
        ReplayInput::new(vec![Some(false)], latest_tick)?.length(),
        1
    );
    for length in [2, 3] {
        assert_eq!(
            message(ReplayInput::new(vec![None::<bool>; length], latest_tick)),
            "replay_input: final dense instant is outside the run time domain"
        );
    }
    for start in [EngineTime::NEVER, EngineTime::MAX_END, EngineTime::FOREVER] {
        assert_eq!(
            message(ReplayInput::<bool>::new(vec![], start)),
            "replay_input: invalid run start"
        );
        assert_eq!(
            message(ReplayInput::new(vec![Some(false)], start)),
            "replay_input: invalid run start"
        );
    }
    Ok(())
}

fn requirement(node: u32, role: BufferRole, scalar: ScalarType) -> BufferRequirement {
    BufferRequirement { node, role, scalar }
}

#[test]
fn typed_bindings_validate_before_any_start() -> Result<()> {
    let required = [
        requirement(0, BufferRole::ReplayInput, ScalarType::I64),
        requirement(1, BufferRole::Capture, ScalarType::I64),
    ];
    validate_bindings(
        7,
        &required,
        &[
            BufferBinding::replay::<i64>(7, 0, 0),
            BufferBinding::capture::<i64>(7, 1, 1),
        ],
    )?;
    assert_eq!(
        message(validate_bindings(7, &required, &[])),
        "replay_input: missing binding"
    );
    assert_eq!(
        message(validate_bindings(
            7,
            &required,
            &[BufferBinding::replay::<i64>(8, 0, 0)]
        )),
        "replay_input: binding belongs to another run"
    );
    assert_eq!(
        message(validate_bindings(
            7,
            &required,
            &[BufferBinding::replay::<bool>(7, 0, 0)]
        )),
        "replay_input: binding role or type mismatch"
    );
    assert_eq!(
        message(validate_bindings(
            7,
            &required,
            &[BufferBinding::capture::<i64>(7, 0, 0)]
        )),
        "replay_input: binding role or type mismatch"
    );
    assert_eq!(
        message(validate_bindings(
            7,
            &required,
            &[BufferBinding::replay::<i64>(7, 9, 0)]
        )),
        "replay_input: binding names an unknown node"
    );
    Ok(())
}

#[test]
fn duplicate_node_bindings_and_capture_writers_are_rejected() {
    let required = [
        requirement(0, BufferRole::Capture, ScalarType::I64),
        requirement(1, BufferRole::Capture, ScalarType::I64),
    ];
    assert_eq!(
        message(validate_bindings(
            7,
            &required,
            &[
                BufferBinding::capture::<i64>(7, 0, 10),
                BufferBinding::capture::<i64>(7, 0, 11),
            ]
        )),
        "capture: node has duplicate bindings"
    );
    assert_eq!(
        message(validate_bindings(
            7,
            &required,
            &[
                BufferBinding::capture::<i64>(7, 0, 10),
                BufferBinding::capture::<i64>(7, 1, 10),
            ]
        )),
        "capture: buffer has multiple writers"
    );
}
