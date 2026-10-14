# Card: Typed Result and diagnostic storage

The outcomes module stores one ordinary tagged result, canonical Error data and
nullable origin metadata using existing global/prepared ownership operations.

Public markers: `ResultValue<T: GlobalValue,E: GlobalValue>`, `Error`, `Context`.
ResultValue::Value is Rust `Result<T::Value,E::Value>`; Error::Value is ErrorValue;
Context::Value is ErrorContext. Each uses three independently typed ValueSlots.
The result tag selects one optional payload and does not imply temporal validity.
No payload in the unselected branch is read. All implement GlobalValue and, when
their descendants permit it, PreparedValue.

`ResultBounds<T: PreparedValue,E: PreparedValue>` exposes field0 bool bounds,
field1 Optional<T> bounds and field2 Optional<E> bounds. `ContextBounds` exposes
String, String and EngineTime field bounds. `ErrorBounds` exposes String,
Optional<String> and Optional<Context> field bounds. Default, Debug and Clone
follow descendant bounds. Include/check/copy preserve whole-owner lifetime and
preflight before mutation, using existing optional layouts.

Successful prepared bounded constructors, selected payload forwarding and Error
data publication allocate nothing through start, evaluation and cleanup. Generic
ordinary helper owning extraction and captured failure retention can allocate;
these profiles are measured independently. No universal failure allocation claim
or runtime type/name lookup is introduced.
