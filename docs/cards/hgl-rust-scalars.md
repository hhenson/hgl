# Card: hgl-rust-scalars

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `scalars` module of `hgl-rust` (`crates/hgl-rust/src/scalars.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Scalar source-to-Rust spelling helpers extracted unchanged from hgl-rust-layouts.
Uses hgl-source and hgl-types; budget 150 source lines. Preserve and re-export
existing scalar helper signatures at their original public paths; list any new
signature here before adding it. Extend the mappings for CivilDateTime, ZoneId,
ZonedDateTime and ZonedTime with exact scalar identity. No runtime/provider work lives here.

Declared enum source types retain nominal identity through generated i64-backed
GlobalValue markers. Enum publication uses one prepared whole-value slot without
structural children. Checked harness capture retains the original declaration
and assigned member number; it never substitutes an ordinary integer.

May use hgl-semantics. literal(&Literal)->String emits scalar values including bytes; values re-exports its existing path. integer_binary(&str,&str,&str)->String emits the established integer policy. bytes_input(&Value)->Option<usize> recognizes direct constructors and pure single-argument, one-return byte helpers fed by a complete atomic i64 list; it never executes or drops effects.
