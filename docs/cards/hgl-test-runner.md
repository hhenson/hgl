# Card: hgl-test-runner

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `test_runner` module of `hgl-compiler` (`crates/hgl-compiler/src/test_runner.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Public hglc test execution using hgl-program checking and emission, a temporary
Cargo package and the workspace runtime crates. run(arguments, root) accepts
FILE, named selectors, --part FILE and --library DIR. Return failure for checking,
build, execution and cleanup failures. Clean temporary package files after a run.
Budget 250 source lines. No additional third-party dependencies.

Ordinary test loads explicit files/parts and libraries once, prepares hgl-reject::Plan, checks surviving source before any execution, selects named cases across both kinds, checks independent rejection cases, and executes surviving named tests despite rejection mismatches. A separate build stage classifies toolchain failures as infrastructure. No rejection-specific command or flag. May use hgl-library-files.

The target module owns executable discovery; imported modules supply production
declarations, and their tests remain in their own scope. Temporary-file, build,
execution and cleanup errors are identified as infrastructure failures.
