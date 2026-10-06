# Card: hgl-fixed-bench

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `fixed` binary of `hgl-bench`; its budget and "may use" list are held per module by `cargo xtask ci`.

Measure native Rust graph execution against the paired C++ fixed-collection
scenarios. May use `hgl-types`, `hgl-store` and `hgl-kernel`; no public API.
Budget: 250 source lines. Build in release mode; time execution only.

`owned` and `assembled` publish four scalar leaves of TSL[TSB] each cycle.
`reference` alternates two whole aggregates whose fields differ by 100.
Every executable checks the checksum and sink count before reporting time.
The C++ baseline uses public native wiring. Measure fresh processes, pinned
to one core on the validation host, and report median/MAD for both halves.
