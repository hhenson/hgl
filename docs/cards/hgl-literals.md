# Card: hgl-literals

The `literals` model is in `crates/hgl-value-model/src/literals.rs`; source parsing is in `crates/hgl-source/src/literals.rs`. Their manifests retain informational line targets and enforce module dependencies.

Source scalar values and unresolved provider-dependent literal recipes. Uses
hgl-calendar, hgl-type-shape and hgl-time-values; budget 250 source lines.

Model owns the retained Literal enum, re-exported by hgl-source. Numeric token
conversion remains at `hgl_source::literals::numeric`, outside Model. Add Literal::CivilDateTime(i64), TimeZone(ZoneId),
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

Literal::Enum(EnumType, i64) retains the declaration identity and assigned member
number. It is a constructed ordinary scalar; frontend validation admits declared
members only. The type is Ty::Enum with the same declaration.

Literal::Bytes(Vec<u8>) is a constructed exact scalar value, never a new lexer token. It preserves uninterpreted contents and canonical bytes identity through cold checked data.
