# Negative tests

Status: accepted

`hglc test` is the Rust backend spelling of the normative `hgl test` command.
It checks HGL, builds an emitted Rust runner using Cargo, and executes named tests.
Both source/build failures and failed tests return nonzero. Executed failures
print `qualified_test_name ... FAILED`; build failures never print that marker.

From the checkout, run the shared runtime specimen (spec commit `1db4c1f`):

```sh
cargo run -p hgl-compiler --bin hglc -- test examples/negative-tests/runtime.hgl \
  --library external/hgraph_std/hgl/hgraph \
  --part native/stdlib/interfaces.hgl \
  --part native/stdlib/rust.hgl
```

Tests belong to the target module; imported library modules supply dependencies.
Optional positional names select short or qualified test names; unknown names
fail before building. `--library DIR` follows existing library-file collection.
A local Cargo toolchain and this checkout's runtime crates are required. Native
provider definitions still need the existing `emit-tests` embedding interface.

The runtime expectation matches only an originating structured execution code.
Normal completion, another error, assertion failure and failed cleanup fail.
Earlier ordinary writes survive and each evaluated graph is torn down normally.

Standalone `# expect-error(category, "code")` annotations identify rejection
cases during ordinary `hglc test FILE` runs. A case owns its enclosing named test
or declaration. All rejected owners are excluded from executable compilation;
each selected case is checked independently against surviving declarations.
Names select both executable and rejection tests, while declaration-owned cases
always run. Parts preserve their own filename and line locations. Imported
library comments do not suppress dependency errors.

`hglc test external/hgraph_spec/language/examples/reject/mixed.hgl` runs both
kinds of test in one file. Invalid metadata or surviving-source errors stop
admission; an expectation mismatch fails its case while valid selected tests
continue. Reports distinguish executed tests and rejection cases. Other compiler
commands retain ordinary checking and treat annotations as comments.

Native provider definitions still use the existing emitted-runner embedding
interface. Unsupported compiler constructs remain uncoded failures and cannot
satisfy a catalogued expectation.
