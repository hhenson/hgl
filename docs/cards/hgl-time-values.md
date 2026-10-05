# Card: hgl-time-values

Status: accepted

Pure temporal scalar data; no dependencies; budget 250 source lines. Move
EngineTime, EngineDelta, Date and Time here unchanged and re-export them from
hgl-types. Existing engine range and checked arithmetic contracts are unchanged.

Add CivilDateTime(i64), with from_micros and micros, and owned ZoneId(String),
with from_validated_name, as_str and fallible try_clone. Add ZonedDateTime with
private EngineTime, ZoneId and i32 offset fields; expose from_validated_parts,
instant, zone, offset_seconds and fallible try_clone. All expose Debug, Default,
Clone, equality and Hash. CivilDateTime also exposes Copy and chronological
ordering; zone and zoned values do not expose ordering.

Construction from validated parts is an unchecked native adapter boundary:
callers establish the spec's validation first. Default exists for empty physical
slots and never publishes a source value. Zone identity is exact owned text;
zoned identity includes instant, name and offset. Values contain no provider,
reference count or run-local zone index and survive context/graph teardown.

Tests cover exact alias identity, zoned structural equality, independent owned
copies, civil ordering and unchanged engine time boundaries.

`from_validated_name` consumes String; `from_validated_parts` consumes
EngineTime, ZoneId and i32. Both fallible `try_clone` methods return
`Result<Self, std::collections::TryReserveError>`.
