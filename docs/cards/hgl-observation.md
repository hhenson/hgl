# Card: hgl-observation

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `observation` module of `hgl-store` (`crates/hgl-store/src/observation.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Immutable borrowed temporal source views for prepared publication and recording.
Uses bindings, columns, types, atomic, rolling and keys; budget 100 source lines.
No ownership, allocation, unsafe code or third-party dependencies.

Observation exposes borrowed scalar columns, temporal bindings, atomic and rolling
arenas and exact key tables. scalar<T>(InputId) checks validity and borrows the
statically selected scalar column without an owning copy. The prepared Store
facade reexports this type so existing observation clients retain their API.
Keeping source views separate from mutation authority permits scalar and rolling
text composition to share exactly the same generated read callbacks.

Acceptance: existing prepared capture tests and full-evaluation rolling text
composition allocation/readiness tests in debug and release.
