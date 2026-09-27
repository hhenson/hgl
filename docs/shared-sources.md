# Shared sources

Language/runtime rules, HGL examples and expected traces belong to
[hgraph_spec](https://github.com/hhenson/hgraph_spec). Python/C++ experiments
and recorded observations belong to
[hgraph_spec_audit](https://github.com/hhenson/hgraph_spec_audit). Portable
standard-library HGL belongs to [hgraph_std](https://github.com/hhenson/hgraph_std).
Compiler/runtime internals and target implementations stay here.

After cloning or changing dependency pins, run:

```sh
python3 tools/shared_artifacts.py
python3 tools/shared_artifacts.py --check
```

Git submodules in `external/` pin the source versions. `shared-artifacts.json`
maps them to existing compiler, test and documentation paths. Those copies
are ignored build inputs. The tool refuses to overwrite edited copies; move
such edits into the owning submodule and submit them there first. Commit
updated submodule pins with the corresponding implementation change. Use
`--offline` after a recursive clone or when dependencies are already present.

Release source archives must contain initialized dependencies and materialized
inputs when building the language tools or documentation. Core runtime builds
without the language component do not need this setup. Compiler-specific
fixtures and generated implementation code remain local.
