# Card: hgl-rust-checked-data

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `checked_data` module of `hgl-rust` (`crates/hgl-rust/src/checked_data.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Serialize already-checked test IR as Rust source for the generated test host.
Uses hgl-source, hgl-rust-ir and hgl-harness-ir; budget 250 source lines.
Expose test(&Test) -> String and ty(&Ty) -> String. Preserve exact nominal and
origin metadata, source-ordered arguments/statements, contextual literal recipes
and prepared binding indices. This performs no expression evaluation, provider
lookup, native value conversion or graph wiring.

hgl-rust-preparation delegates checked data emission here and retains native
conversion and runner emission. Test nested source types, escaped names/literals
and ordered checked forms through compiled generated executables.

Declared enum source types retain nominal identity through generated i64-backed
GlobalValue markers. Enum publication uses one prepared whole-value slot without
structural children. Checked harness capture retains the original declaration
and assigned member number; it never substitutes an ordinary integer.

value(&Value) -> String exposes exact checked expression serialization for the
cold finite collection key seeding owner.

Serialize complete recursive root batches and nominal internal edges explicitly.
Definitions remain canonical and immutable when reconstructed. May use
hgl-rust-enums::metadata for exact enum metadata shared by type/literal emission.

Delegate exact checked type metadata to hgl-rust-type-data and re-export ty.
Serialize BindEval and recursive If steps, presence tests/extractions and closed
sparse Captured data without evaluating any branch or expression.

Serialize Step::Raises with its exact code and recursive lexical block.
