# Compiler tooling evidence

Status: measured, 2026-09-22.

`linux-summary.json` identifies compiler, platform, corpus and experiment source
hashes. `linux-builds.json` contains three cold debug/release builds per feature
set. `linux-execution.json` contains seven timed batches per operation.
`macos-summary.json` records the same source hashes and upstream syntax/arithmetic
validation, including the reference executable's hash. Machine identities and
local filesystem roots are deliberately omitted.

Reproduce with `../run.py --output <directory> --measure`; pass `--reference`
for the existing HGL executable. Dependencies are fixed by the experiment's
Cargo.lock. See the experiment README for precisely what is generated and which
parts of the runtime fixture remain handwritten. This is not the C++ runtime
performance gate or a full language conformance result.
