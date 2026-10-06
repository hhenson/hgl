# Card: hgl-std-native

Rust implementations of the selected shared native scalar interfaces. Uses
`hgl-types`, `hgl-calendar`, `hgl-kernel`, `hgl-store`; budget 700 lines. No third-party
crates. Selection stays beside Rust binding code in `native/stdlib/rust.hgl`;
portable interfaces and operator bodies remain in hgraph_std.

Public free functions use the emitter's `name_argtype...` spelling. Surface:
text `len`, `is_empty`, `contains`, `starts_with`, `ends_with`, `slice`;
`truthy` (f64/str), `bit_and` (i64), `power` (i64/f64), integer shifts,
`round_decimal`; `as_str` for the eight scalar types; date year/month/day,
time hour, datetime year/hour/timestamp, duration days/seconds/microseconds;
`as_float_i64`, `as_int_f64`; numeric/text `add`, integral `div`, `floordiv`,
`mod`; `print_line_str`, `log_info_str`, `raise_error_str`.
`rust.hgl` lists the exact selected overloads. These are scalar functions,
not replacements for HGL stream nodes. Unselected native signatures fail
compilation rather than acquiring an implicit implementation.

Text inputs borrow `&str`; results own their payload. Declared `throws`
functions return `Result<T, Box<NodeError>>`. Logging uses the test host's
stderr service. Formatting and collection growth may allocate; this slice
makes no new performance-parity claim.

Acceptance: embedded shared native tests plus overflow/error paths, signed
floor/modulo, Unicode slicing, exponent spelling and negative durations.

## Dense scalar eval storage

`eval_buffers` provides the storage operations of specification ADR 0016.
Replay and record are ordinary HGL source/sink operators; these Rust types
never schedule, publish, select ticks, advance cursors or pad dense results.
The fresh graph owns typed buffers through stop and owned-result extraction.

Public surface:

- `BufferScalar: hgl_store::Scalar`, with fallible `copy_delta(&self)` returning
  `Result<Self, Box<NodeError>>`; implementations for bool, i64, f64, String,
  Date, Time, EngineTime and EngineDelta. String copies reserve fallibly.
- `ReplayInput<T>::new(Vec<Option<T>>, EngineTime)` validates i64 length and
  legal run start/final dense time strictly before the latest exclusive end;
  `length() -> i64`, `get(i64) -> Result<Option<T>, Box<NodeError>>`.
  Indexed reads return independent owned present payloads or successful
  in-range absence; negative or past-end indices fail. These native methods
  lower HGL `len(replay_input)` and `replay_input[index]`, respectively.
- `Capture<T>::new()`/`Default`, `begin() -> NodeResult`,
  `append(time: EngineTime, delta: &T, evaluation_time: EngineTime) -> NodeResult`,
  `take_ticks(&mut self) -> Result<Vec<(EngineTime, T)>, Box<NodeError>>`.
  Begin is separate from binding. Append validates begun/current/increasing
  time in order, copies before mutation, and keeps previous ticks on failure.
  Extraction transfers ownership; an unbegun capture is an error.
- `BufferRole::{ReplayInput,Capture}`;
  `BufferRequirement { node: u32, role: BufferRole, scalar: ScalarType }`;
  opaque `BufferBinding` with typed `replay::<T>(run, node, buffer)` and
  `capture::<T>(run, node, buffer)` constructors.
- `validate_bindings(run: u64, &[BufferRequirement], &[BufferBinding]) -> NodeResult`
  checks missing/duplicate bindings, exact run/node/role/type, duplicate
  requirements and single capture writer before any graph start.

Binding IDs exist only in the construction-local manifest. Generated nodes
own their static typed fields; no global registry or per-tick type/name lookup
is involved. The compiler checks method phases, supported node shape and
capability escape. Provider methods preserve ADR 0016's ordered error prefixes.
This dense eval profile runs to the fixed latest exclusive end; an eventual
custom run end would require a corresponding provider configuration contract.
Capture buffers and owned text can allocate as test instrumentation; no new
allocation-free or performance-parity claim is made.

Acceptance: all eight scalar payloads; nullable reads and bounds errors; empty/unbegun
capture; repeated begin; timestamp validation and retained captures; independent
owned strings and runs; invalid binding manifests and duplicate writers.

BufferScalar delegates to Scalar::try_clone for every concrete runtime scalar,
including CivilDateTime, ZoneId, ZonedDateTime and ZonedTime. Owned zone identities are
retained independently without provider access.

`midnight_date(Date) -> EngineTime` supplies UTC midnight for an admitted
calendar date to the shared `to_datetime` operator. The backend interfaces part
declares this adapter and the Rust binding part selects its provider. It uses epoch-relative days, with no zone, clock
or DST dependency; admitted years 0001–9999 fit exactly in microseconds.
Acceptance includes epoch/leap/calendar boundaries and the shared tick/silence
conversion test under the pinned std date-conversion contract.
