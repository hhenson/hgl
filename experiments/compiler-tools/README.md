# Compiler tooling experiment

Status: experimental; dependency evaluation authorized, production choices pending.

Compare handwritten and Logos lexers, handwritten and Chumsky parsers,
plain and Rowan source storage, codespan diagnostics, and text versus
quote/syn/prettyplease Rust emission. These are isolated experiment dependencies;
no runtime crate or production compiler depends on them.

The pipeline is Lexer -> Parser -> Checker -> Rust emitter. The bounded grammar
covers module headers, named structs, concise functions, named parameters,
i64/ref/fixed-list/named types, integer/name expressions, parentheses, + and *.
It is not the complete HGL grammar. Other syntax must fail, never disappear.

Before implementation, the acceptance cases are:

- Preserve every source byte, including comments, CRLF and erroneous tokens.
- Parse multiline signatures, continuation after => and +, nested generic
  closers, precedence and parentheses identically with both parsers.
- Recover two broken declarations and retain a following valid function;
  preserve unexpected tokens and explicit missing-token diagnostics.
- Check names, return types and recursive type formation. Explicit ref<ref<T>> is invalid. After substituting a reference type for T,
  ref<T> collapses to that reference; a collection between reference wrappers is not collapsed.
- Emit scalar bodies and recursive shapes through both emitters. Compile each
  artifact independently and run the same literal assertions. Exercise emitted
  shapes under peered/assembled binding, REF replacement and child retirement.
- Existing accepted runtime traces remain the semantic baseline. This spike
  does not claim full source-to-graph lowering or reference-runtime parity for
  unsupported language features.

Measure both parser implementations on identical pre-tokenized valid/error
inputs; also measure complete lexer+parser paths. Record cold debug/release
builds separately from warmed execution. Numbers are experiment measurements,
not the runtime's C++ performance gate. Never choose a dependency on speed alone.

Source contracts: hgraph `language/src/syntax/{lexer,parser}.h` and
`language/docs/{developer-guide/syntax-and-semantics,design/compiler-architecture}.md`.
Literal corpus expectations precede candidate execution. Upstream compiler
checks validate the accepted syntax; observed disagreements stay in the report.

Run `python3 experiments/compiler-tools/run.py --output /tmp/compiler-tools`.
Add `--reference /path/to/hgl` for upstream syntax and arithmetic checks;
`--measure` performs three cold builds per profile/feature set and seven warm
execution samples. Cargo's dependency cache is warm; build directories are fresh.
Compiler wrappers are disabled for cold builds. No benchmark threshold is enforced. The runtime fixtures are copied unchanged
except that their shape constructor uses the emitted `shape_forward()` result.
The emitted arithmetic body runs inside a fixed harness node. This proves
body/shape emission, not generation of those support nodes or graph templates.

Rowan is tested as lossless token storage only, not as a complete recovered
syntax-tree implementation. Checker diagnostics currently use strings; source
labels are exercised on parser diagnostics. Generic substitution is supplied to
the checker directly; generic call inference is outside this spike.
