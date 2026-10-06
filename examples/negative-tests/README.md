# Negative tests

Status: accepted

`hglc test` is the Rust backend spelling of the normative `hgl test` command.
It checks HGL, builds an emitted Rust runner using Cargo, and executes named tests.
Both source/build failures and failed tests return nonzero. Executed failures
print `qualified_test_name ... FAILED`; build failures never print that marker.

From the checkout, run the shared runtime specimen (spec commit `1db4c1f`):

```sh
cargo run -p hgl-compiler --bin hglc -- test examples/negative-tests/runtime.hgl \
  --part external/hgraph_std/hgl/hgraph/replay_record.hgl \
  --part external/hgraph_std/hgl/hgraph/impl/replay_record.hgl \
  --part external/hgraph_std/hgl/hgraph/control.hgl \
  --part external/hgraph_std/hgl/hgraph/impl/control.hgl
```

Optional positional names select short or qualified test names; unknown names
fail before building. `--library DIR` follows existing library-file collection.
A local Cargo toolchain and this checkout's runtime crates are required. Native
provider definitions still need the existing `emit-tests` embedding interface.

The runtime expectation matches only an originating structured execution code.
Normal completion, another error, assertion failure and failed cleanup fail.
Earlier ordinary writes survive and each evaluated graph is torn down normally.

Compile-rejection fixtures use `hglc test --reject FILE`; no names, module parts,
Rust build or graph execution are admitted in this mode. Each actual standalone
`# expect-error(category, "code")` comment matches one primary source diagnostic
on the immediately following physical line, with exact file, category and code.
The same whole-source admission checks run before ordinary compilation.

For example, `hglc test --reject external/hgraph_spec/language/examples/reject/rolling-size-kind.hgl`
checks the intentional rejection specimen. Those fixtures are separate from
ordinary successful example suites. Unknown metadata, unexpected or uncoded
errors, wrong locations and successful source checking all fail.

The standalone `check` command retains its existing declaration-checking slice;
program compilation resolves executable graphs. Rejection additionally checks
concrete declarations and named test bodies through that same program resolver.
Generic failures become concrete when invoked; unsupported constructs remain
unexpected uncoded failures, so they cannot satisfy catalogue expectations.
