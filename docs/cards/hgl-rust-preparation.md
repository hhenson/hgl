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

Declared enum source types retain nominal identity through generated i64-backed
GlobalValue markers. Enum publication uses one prepared whole-value slot without
structural children. Checked harness capture retains the original declaration
and assigned member number; it never substitutes an ordinary integer.

Map Keyed(Value,Value) entries and Add/Remove(Value) decode materialized ordinary
keys by exact K; capture re-encodes complete typed keys independently of runtime
membership IDs. Field and fixed-list Child indices remain i64.

Optional struct encode/decode preserves missing field indices as unset and
present fields as independently retained payloads. Decode rejects missing
required fields. Encoding an all-unset struct still creates a present Construct;
only dense eval silence remains an absent sequence cell.

recursive_markers(plan)->String delegates typed cold recursive conversion methods
to hgl-rust-recursive-convert, an allowed dependency. decode/encode dispatch
recursive ordinary values to their statically selected member methods; recursion
occurs through finite payloads, never recursive schema expansion.
