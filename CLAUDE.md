# hgl

A new compiler back end and a compact runtime for HGL, in Rust. The experiment
is the method as much as the code: can a described design be held to "clean,
compact, understandable" when agents write it?

## Done means

`cargo xtask ci` passes. Run it before reporting any work as finished, and
report a failing or skipped gate as exactly that.

## Rules the tools cannot check

- **The language is specified upstream**, in `../hgraph/language/docs`. Do not
  invent or adjust HGL semantics here. A needed extension is proposed for
  hgraph first, made to work there, and only then used here.
- **The reference runtime is the oracle.** When behaviour is unclear, find out
  what hgraph does and record it as a corpus case; do not guess.
- **A new third-party crate needs the owner's agreement.** Ask first, then add
  it once to `[workspace.dependencies]`.
- **Never buy a green build.** No `#[allow]`; `#[expect(.., reason)]` only with
  a real reason; never raise a lint threshold or a line budget, never delete
  or weaken a test, never stub a function to get past a gate. If a gate is
  wrong, say so and stop.
- **Write Rust its owner can read.** The owner's first language is C++. Prefer
  plain structs, enums, functions and `match`. No `macro_rules!`, no
  procedural macros of our own, no trait or lifetime cleverness where a
  simpler shape exists. Fewer lines is a goal; terse is not.
- **Comments say why, never what.** No narration, no change history, no
  explanation of a workaround — fix the cause.

## Where things are

- `docs/decisions/` — what is settled. `docs/explorations/` — what is not.
- `docs/guides/rust-practices.md` — the practices behind this setup.
- `docs/README.md` — conventions for the docs themselves.
