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
  a real reason; never raise a lint threshold or a crate's `line-budget`
  (`[package.metadata.hgl]`, held by `cargo xtask ci`), never delete
  or weaken a test, never stub a function to get past a gate. If a gate is
  wrong, say so and stop.
- **Write Rust its owner can read.** The owner's first language is C++. Prefer
  plain structs, enums, functions and `match`. No `macro_rules!`, no
  procedural macros of our own, no trait or lifetime cleverness where a
  simpler shape exists. Fewer lines is a goal; terse is not.
- **Nothing obviously slow.** The runtime is judged against hgraph's C++
  runtime, within 5% (`docs/decisions/0002`). On the per-tick path: no
  allocation, no lookup by name, no `Rc`/`RefCell`/`Arc`/`Mutex`, no type
  test per value, no work proportional to the size of the graph, no copying
  a value to move it, no writing twice. The full list and the reasons are in
  `docs/explorations/0009-designing-for-speed.md`. `unsafe` is allowed only in
  the store crate (`docs/decisions/0003`). A performance figure counts only
  if it was measured on the validation host against its C++ baseline.
- **Comments say why, never what.** No narration, no change history, no
  explanation of a workaround — fix the cause.

## Where things are

- `docs/decisions/` — what is settled. `docs/explorations/` — what is not.
- `docs/runtime_spec/` — what the runtime is; rules are numbered (`GRF-11`).
- `docs/cards/` — the contract a crate is built from. Add no public item a
  card does not list; if one is needed, the card is wrong — say so.
- `bench/` — benchmark pairs and recorded results.
- `docs/guides/rust-practices.md` — the practices behind this setup.
- `docs/README.md` — conventions for the docs themselves.
