# Card: hgl-library-files

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `library_files` module of `hgl-program` (`crates/hgl-program/src/library_files.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Discover and read explicit source files and library roots. Standard library only;
budget 100 source lines. Public `sources(files: &[PathBuf], libraries: &[PathBuf])
-> Result<Vec<(String, String)>, String>` preserves explicit-file order, sorted
recursive library discovery, test/example directory exclusions and path-qualified
I/O diagnostics. This boundary neither parses nor executes HGL.

Acceptance: existing hgl-program file/library loading and shared-suite coverage.
