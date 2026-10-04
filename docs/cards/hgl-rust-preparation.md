# Card: hgl-rust-preparation

Status: accepted

Emit cold, schema-specialized preparation and test execution code. Uses
hgl-source, hgl-rust-ir and hgl-harness-ir; budget 300 source lines. Expose
decode(ty: &Ty, value: &str) -> String, encode(ty: &Ty, value: &str) -> String,
and emit_test(test: &Test, plans: &[Plan]) -> String. Reuse existing backend
helpers through hgl-rust-layouts and hgl-rust-values dependencies.

Generated conversions bridge constructed owning checked values and exact native
configurations/captures, preserving recursive nominal and delta origins, empty
payloads and absence. They never resolve literal recipes. A native RunContext
exists before test setup and selects the bundled conformance catalog; prepared configurations enter a fresh graph through
typed construction factories. No hidden global keys, replay provider lookup,
compiler TZDB, expression re-evaluation or runtime choice of temporal shape.

May use hgl-rust-checked-data for checked-IR source serialization; this crate
retains native conversions and runner construction, with its budget unchanged.
