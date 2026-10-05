# Card: hgl-global-value

Typed representations of ordinary primitives, required-field nominal structs,
and homogeneous lists. Uses `hgl-types`, `hgl-columns` and `hgl-global-arena`;
budget 200 lines. No third-party dependencies or unsafe code.

`GlobalValue` names a generated nominal marker's `Value` (ordinary owning Rust
representation) and Copy `Slots` (typed field handles). Scalar types implement
this trait directly. Struct implementations delegate in declaration order.
`ValueColumns`, `Capacity`, `Layouts` and `ListData` re-export the arena vocabulary.

The trait defines:

- `const WIDTH: usize`: immediate flattened positions, counting each scalar or
  list descriptor once; list elements have separate layouts.
- `schema() -> OrdinaryType`: exact nominal/container identity, inspected only
  during construction.
- `slots(&mut &[usize]) -> Slots` and `flatten(Slots, &mut [usize])`: reconstruct
  or serialize typed positions in declaration order, never payloads.
- `retain(&Value) -> NodeResult<Value>`: independent recursive owning retention.
- `prepare(&Value, &mut Capacity, &mut Layouts) -> NodeResult`: validate payload
  constraints, count slot demand, and acquire every list layout buffer.
- `read(&ValueColumns, Slots) -> NodeResult<Value>`: explicit owning extraction.
- `commit(&mut ValueColumns, Slots, Value, &mut Layouts)`: infallibly move a fully
  retained/prepared value into existing root positions.
- `install(&mut ValueColumns, Value, &mut Layouts) -> Slots`: infallibly install
  new descendants after all arena and layout capacity has been acquired.
- `release(&mut ValueColumns, Slots)`: recursively reclaim typed descendants;
  pre-reserved free pools make release allocation-free.

`ValueSlot<T: GlobalValue>` is an opaque Copy/Debug typed handle with
`bind(&mut &[usize]) -> Self`, `install(&mut ValueColumns, T::Value,
&mut Layouts) -> Self`, `fields() -> T::Slots`, `read(&ValueColumns)`,
`commit(&mut ValueColumns, T::Value, &mut Layouts)`, `release(&mut ValueColumns)`
and `flatten(&mut [usize])`. Marker implementations are compiler/native provider
contracts. Malformed layouts or slots from another owner are caller errors.
`allocate(&OrdinaryType, &mut ValueColumns, &mut Vec<usize>) -> NodeResult`
prepares root positions before start, leaving presence to the owning entry.

Rules: ADR 0016 exact nominal/container types, value-mutability lexical access,
LIST-RETAIN/ERROR and VAL-17 independent ownership. Borrow/projection copies only
handles. Retention, preparation and every required capacity acquisition complete
before any live leaf changes. Physical capacity may grow during a failed
reservation; logical values and presence remain unchanged. Commit neither copies
payloads nor allocates. List replacement keeps its descriptor identity and releases
old descendants through their statically selected marker.

No hook-time schema inspection, key lookup, type dispatch, reference counting or
borrow registry. The compiler checks lexical permission/lifetime conflicts.

Acceptance lives in hgl-store and hgl-describe: nested primitive/struct/list
projection, exact-type preflight, absence, independent retention, stable root
replacement, failed retention/reservation preserving every preceding field, no
borrow allocations, and bounded arena storage under repeated replacement.
Mutants: erase nominal identity; copy payload at borrow; commit a field before
later preparation fails; project the wrong slot; detach root on replacement;
forget typed descendant reclamation.

Ordinary tuple preparation uses `OrdinaryType::Tuple` and positional child
layouts. Generated tuple GlobalValue implementations retain, prepare, commit
and reclaim recursively under the same ownership contract as nominal structs.

CivilDateTime, ZoneId, ZonedDateTime and ZonedTime are concrete typed scalar implementations.
Zone-bearing values retain exact owned names fallibly at retention boundaries;
prepared borrowed projections neither allocate nor consult a provider.

GlobalValue::PREPARED_SCALAR defaults to false. Generated enum markers set it
true and use Value=i64, Slots=usize, WIDTH=1 and OrdinaryType::Enum(identity).
They delegate typed slot operations to i64 while preserving the nominal schema.
Only such enum roots are initialized physically during endpoint construction;
no default enum member is published. Ordinary allocation recognizes Enum as one
i64 physical leaf. Retention and complete nested payload rules remain unchanged.
