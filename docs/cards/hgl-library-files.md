# Card: hgl-library-files

Discover and read explicit source files and library roots. Standard library only;
budget 100 source lines. Public `sources(files: &[PathBuf], libraries: &[PathBuf])
-> Result<Vec<(String, String)>, String>` preserves explicit-file order, sorted
recursive library discovery, test/example directory exclusions and path-qualified
I/O diagnostics. This boundary neither parses nor executes HGL.

Acceptance: existing hgl-program file/library loading and shared-suite coverage.
