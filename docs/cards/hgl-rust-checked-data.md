# Card: hgl-rust-checked-data

Status: accepted

Serialize already-checked test IR as Rust source for the generated test host.
Uses hgl-source, hgl-rust-ir and hgl-harness-ir; budget 250 source lines.
Expose test(&Test) -> String and ty(&Ty) -> String. Preserve exact nominal and
origin metadata, source-ordered arguments/statements, contextual literal recipes
and prepared binding indices. This performs no expression evaluation, provider
lookup, native value conversion or graph wiring.

hgl-rust-preparation delegates checked data emission here and retains native
conversion and runner emission. Test nested source types, escaped names/literals
and ordered checked forms through compiled generated executables.
