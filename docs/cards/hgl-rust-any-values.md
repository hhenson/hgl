# Card: hgl-rust-any-values

The any_values emitter compiles ordinary box codecs, static identities, capability descriptors and cold capacities. It may use cold_validation, deltas, layouts, collections, families, scalars and value_convert; initial budget 600 lines within the unchanged hgl-rust cap.

Public surface: layouts, constructor, construct, contains, capture_equal, limits, publication, key_preparation, comparison, prepared_native and prepared_source. Internal name emits collision-free identifiers. The closed descriptor table includes every admitted ordinary payload reached by graph hooks, configurations and harness setup/input/expectations. It is not a builtin-only variant table.

Cold owning construction and capture conversion may allocate and use source names. Prepared native/source writers count encoded width and then write into reserved storage. Both select actual concrete family member identity and flatten boxes, with no per-tick name lookup or type checking. Type capabilities are compiled before execution, including empty composite payloads. Capture equality propagates capability failures through the test harness.

Acceptance: pinned shared Any suites and generated measured probes for family locals, complete atomic aggregate inputs, independently retained box locals and nested inline boxes appended to global entries.

Nested codecs write typed descriptor metadata with owner-relative headers. Decode and cold capture follow the producing descriptor, including types absent from the consumer's generated layouts. Every descriptor owns a fallible cold capture callback for its checked IR carrier; incompatible carriers report construction errors. Generated recursive/list capture propagates those errors. No local plan indices remain in retained data or capture dispatch.

The codegen module uses checked_data for cold Native capture metadata. Compiled
copy codecs preserve retained descriptor context; fallible readers preserve
Native preflight errors. Producer-owned capture moves each retained native owner
into checked data without repeating the provider copy.

The private cold_validation emitter (initial allowance 160) may use any_values,
layouts, collections and native_values. It generates complete typed validators
for scalar widths and UTF-8, exact timezone provider membership, time-of-day ranges and zoned datetime offsets at their instant, optional presence, fields, sequences, families,
delta storage and nested descriptor contexts. Native checks use the selected
provider preflight; opaque formats remain provider-owned. Temporal cold recipes use the same bundled RunContext selected for generated target preparation, preserving alias spelling. All framing and
preflight validation finishes before cold decoding copies or captures owners.
Trusted prepared codecs retain their existing operation and allocation costs.

Result and Error boxing use the private any_outcomes emitter (informational
200-line target; may use any_values, layouts and constructor_fields). The main
Any emitter and cold_validation may use it. Result encodes one exact tag and its
selected typed payload. Error encodes message, nullable code and the complete
nullable context; provenance ownership is independent of diagnostic equality.
Native writers, prepared slot writers, copying, cold reading/validation and
width bounds share this encoding. Layout discovery may use constructor_fields
for the existing diagnostic field schema, including its internal Context marker.

AnyOperation::Result delegates equality only to the selected payload;
AnyOperation::Error compares borrowed message and nullable-code frames, ignoring
retained context. No decoding or owning clone is needed for equality. Neither
layout invents hash or ordering. Cold validation rejects invalid tags, truncated
or extra frames, invalid presence and malformed descendants before decoding.
No source unboxing, reflection or prescribed text representation is added.

Outcome boxes downstream of captured/runtime-supplied diagnostics have no cold
width bound. Their Any snapshots, global staging and outputs retain the ordinary
owning policy and may allocate. Finite bounded literal/replay/generator controls
keep zero-allocation startup, successful ticks, equality/copy and cleanup.
