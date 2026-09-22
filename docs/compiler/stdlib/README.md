# Initial port evidence

Status: four reasoned scenarios accepted unanimously; Rust fresh-run specimens implemented.

[Cases](cases.json) were written before the Rust implementations. Their rules
are in [node authoring](../node-authoring.md). For these traces, no Python/C++
variation needs a ruling. Each of the 23 cells asserts publication or absence
of a tick, not merely a current value.

| Evidence | Result |
|---|---|
| Released Python hgraph 0.5.41 | 4/4 scenarios, 23/23 cells |
| Current C++ operators through the Python API | 4/4 scenarios, 23/23 cells |
| Rust replay/node/record graphs | Same four tables; debug and release gates |
| HGL specimens through reference `check` | Accepted |
| HGL specimens through reference `test` | Blocked: process aborts in `NodeCheckpointIdentity` destruction before reporting tests |

The last row is a [toolchain failure](hgl-reference.json), not an output disagreement or a pass.
It reproduced with both available local reference compiler builds. Generated
native code and its loaded runtime may be incompatible; that cause is not yet
proven. The directly exercised Python and C++ operators agree with reasoning.
Do not claim HGL-to-Rust source compilation: these Rust nodes are handwritten.

`nodes.hgl` transcribes the upstream `bit_and<i64>`, `sample<T>` and
`dedup<i64>` bodies. It renames local wrappers, specializes sample to i64 and
colocates the existing native bitwise helper to avoid a package dependency.
Upstream source revision: `73cc53c97c54079e245e538ae61a3709012ba933`.
`cases.hgl` and the Rust test table are generated from the same reasoned JSON.

```sh
python tools/stdlib_fixtures.py --check
python -m unittest discover -s docs/compiler/stdlib -p 'test_*.py'
python-hgraph -I docs/compiler/stdlib/replay.py --engine python --output python.json
cpp-hgraph -I docs/compiler/stdlib/replay.py --engine cpp --output cpp.json
hgl check docs/compiler/stdlib/nodes.hgl --part docs/compiler/stdlib/cases.hgl
hgl test docs/compiler/stdlib/nodes.hgl --part docs/compiler/stdlib/cases.hgl
cargo test -p hgl-native -p hgl-stdlib
```

`python-hgraph` and `cpp-hgraph` denote separately installed interpreters.
Replay refuses the wrong engine and records package and binary hashes. Review
fresh observations before replacing the checked-in evidence.

Native-view tests cover unbound/idle inputs, peered and assembled TSL/TSB,
nested immediate-child `all_valid`, cached invalidation times and scoped REF
retirement and TSD removed-member expiry. Their expectations reuse TS-9/14/25/26 and the accepted fixed
collection corpus; they do not claim new unanimous cross-runtime agreement.
A peered structure is invalidated at its root; removing the final valid child
invalidates an assembly. Retired child storage remains readable during its
retirement cycle, then expires at the next cycle.

[Mutation evidence](mutants.json): all six card mutations were rejected on an
isolated copy. The original workspace remained unchanged.
