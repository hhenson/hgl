# Card: hgl-literals

Status: accepted

Source scalar values and unresolved provider-dependent literal recipes. Uses
hgl-calendar, hgl-type-shape and hgl-time-values; budget 250 source lines.

Own the existing Literal enum and numeric token conversion, re-exported by
hgl-source. Add Literal::CivilDateTime(i64), TimeZone(ZoneId),
ZonedDateTime(ZonedDateTime) and ZonedTime(ZonedTime); Literal::ty returns the
exact source type.
TemporalLiteral::{TimeZone(String), ZonedDateTime { instant_micros: i64,
zone: String, offset_seconds: i32 }, ZonedTime { time_micros: i64, zone: String }}
stores an unvalidated recipe and exposes ty.
ParsedLiteral::{Value(Literal), Contextual(TemporalLiteral)} distinguishes them.
numeric(text: &str, negative: bool) returns Result<Option<ParsedLiteral>, String>.

Check grammar, widths, calendar/range, explicit offset and zone syntax without
consulting a provider. Preserve exact names and microseconds. A contextual
recipe is not a constructed scalar or a foldable constant. Zoned-time literals
validate the clock without accepting an offset. Reject offset-free named zoned
datetimes with the spec's resolution hint. Parsing follows the pinned spec, not backend parsing.

ParsedLiteral::ty() -> Ty returns the type of its fixed value or recipe.
