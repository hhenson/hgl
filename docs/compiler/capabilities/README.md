# Inferred capabilities

A call silently adds the callee's injectable requirements to the caller.
Propagation is transitive, independent of declaration order, and deduplicates
explicit requests. Selected native implementation parts supply requirements; descriptor imports
retain the selection.
Injection changes neither the source argument list nor the activation policy.
Missing context and forbidden phases remain errors; omission of `inject` is not.

For input ticks `2, _, 2, 3`, an identity helper using its caller's logger must
return ticks `2, _, 2, 3` and execute three times. The idle cycle does not call
the helper; the equal second `2` still does. A chain of two helpers has the
same result and uses the enclosing node's context. Reading the evaluation clock
in a helper yields cycle offsets `0, _, 2, 3`, with no extra evaluation.

[Python](python.json) and [C++ runtime](cpp.json) agree with this reasoning.
The [reference replay](replay.py) explicitly forwards the logger through the
helpers; these runtimes do not implement HGL's capability inference. Compiler
checks must separately prove silent inference, deduplication, imports, lifting,
and rejection of calls lacking a valid context. Generated C++ tests exercise
native and HGL helpers; Rust tests exercise the generated native trait with a
borrowed logger, not generated Rust nodes.

```sh
python-hgraph -I replay.py --engine python --output python.json
cpp-hgraph -I replay.py --engine cpp --output cpp.json
```

No variation was found in the reference ticks or helper-call counts.

Moving requirements from the shared signature to a selected part changes no
tick expectation above. Target selection, duplicate providers and graph/node
shape are compiler checks, not Python/C++ runtime variations.
