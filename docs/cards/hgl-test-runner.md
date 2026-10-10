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

The executable CLI test host supplies the existing hgl-stdlib standard native
facade and Provider implementation adapter. Implicit logger service forwarding
uses log_info_str from that same host, including calls inside ordinary value
helpers. Custom native implementations remain an embedding responsibility; the
CLI selects no custom provider or arbitrary code dynamically. No source callable
identity, evaluation order, service privilege or external dependency changes.
Acceptance includes the selected published Tuple order examples with captured
actual INFO lines exactly 2,1,3,2,1, alongside successful source result assertions.
