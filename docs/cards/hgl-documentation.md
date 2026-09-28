# Card: hgl-documentation

Owned source documentation and reStructuredText output, independent of compiler
ASTs, runtime types and code generation. Budget: 250 source lines. No dependencies.

Public surface: `Documentation` carries the HGL name, declaration signature,
normalized Google-style/reST text, part and source byte span. `normalize` removes
comment framing and common indentation. `validate` checks parameter documentation
for the Rust compiler's currently nongeneric declaration subset. `render` emits
reST from documentation records without executing directives or interpreting them
as Markdown. More language forms need corresponding checker validation first.

The compiler owns attachment and declaration checking; this crate owns markup
preservation and output. Tests cover Unicode, CRLF, relative indentation, math,
diagrams, unknown keys, and separate interface/implementation docs. Do not resolve
HGL symbols, read source files or invoke a renderer here.
