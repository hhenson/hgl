# Card: hgl-calendar

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `calendar` module of `hgl-types` (`crates/hgl-types/src/calendar.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Calendar literal parsing and native projections. Uses `hgl-types`; budget 400
lines. Calendar payloads are independent of the engine's scheduling bounds.

Surface: `DAY`; `date`, `time`, `datetime`, `duration` return checked scalar
values from strings; `components(Date) -> (i64,i64,i64)`;
`date_text`, `time_text`, `datetime_text`, `duration_text` return display text.
Date is epoch-relative days; Time is microseconds after midnight. UTC datetime
literals require Z. Integral compound durations use checked arithmetic.

Acceptance: Gregorian leap boundaries, pre-epoch dates, fractional seconds,
overflow rejection and Python-compatible negative duration normalization.

## Temporal scalar preparation

Add civil_datetime(&str) -> Result<CivilDateTime, String> and
offset_datetime(&str) -> Result<(EngineTime, i32), String>, using hgl-time-values.
Validate fixed-width calendar fields, explicit offset and representable civil/UTC
range with checked arithmetic. Existing Z datetime syntax remains unchanged.
No provider lookup or alias normalization occurs here.
