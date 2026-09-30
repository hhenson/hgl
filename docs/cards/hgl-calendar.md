# Card: hgl-calendar

Calendar literal parsing and native projections. Uses `hgl-types`; budget 400
lines. Calendar payloads are independent of the engine's scheduling bounds.

Surface: `DAY`; `date`, `time`, `datetime`, `duration` return checked scalar
values from strings; `components(Date) -> (i64,i64,i64)`;
`date_text`, `time_text`, `datetime_text`, `duration_text` return display text.
Date is epoch-relative days; Time is microseconds after midnight. UTC datetime
literals require Z. Integral compound durations use checked arithmetic.

Acceptance: Gregorian leap boundaries, pre-epoch dates, fractional seconds,
overflow rejection and Python-compatible negative duration normalization.
