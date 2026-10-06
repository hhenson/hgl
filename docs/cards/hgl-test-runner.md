# Card: hgl-test-runner

Status: accepted

Public hglc test execution using hgl-program checking and emission, a temporary
Cargo package and the workspace runtime crates. run(arguments, root) accepts
FILE, named selectors, --part FILE and --library DIR. Return failure for checking,
build, execution and cleanup failures. Clean temporary package files after a run.
Budget 250 source lines. No additional third-party dependencies.

`test --reject FILE` delegates to hgl-reject before any source execution setup;
reject mode accepts exactly that path, with no names or module-part options.
