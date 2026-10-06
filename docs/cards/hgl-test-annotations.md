# hgl-test-annotations

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `test_annotations` module of `hgl-compiler` (`crates/hgl-compiler/src/test_annotations.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Lexical expectation metadata for ordinary mixed HGL test runs. annotations reads
actual standalone line comments and returns Expectation records with the exact
following line, category and catalogue code. Empty metadata is valid. Strings
and block comments are ignored, malformed annotations fail admission. May use
hgl-source and hgl-diagnostics; never classify compiler errors by message.
