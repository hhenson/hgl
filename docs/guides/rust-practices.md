# Rust practices for this repository

Status: accepted (the setup exists); the reading guide is a living aid

Written for an owner whose first language is C++. Three parts: what the setup
is and why, how to read Rust coming from C++, and what to look for in a diff.

## The setup

| File | What it does | C++ equivalent |
|---|---|---|
| `Cargo.toml` (root) | The workspace: members, shared dependency versions, the lint policy, build profiles | top-level `CMakeLists.txt` + `conanfile.py` + `.clang-tidy` |
| `Cargo.lock` | Exact versions of everything resolved. Committed, because this builds applications | Conan lockfile |
| `rust-toolchain.toml` | Pins the compiler. rustup reads it and installs that version | a pinned compiler image |
| `rustfmt.toml` | Formatter options. Deliberately almost empty | `.clang-format` |
| `clippy.toml` | Thresholds for clippy lints (function length, nesting) | `.clang-tidy` options |
| `deny.toml` | Which licences, sources and duplicate versions dependencies may have | — |
| `.cargo/config.toml` | Defines `cargo xtask` | — |
| `xtask/` | Repository automation as a Rust program — no shell, works on Windows | `tools/*.py`, CMake scripts |
| `.github/workflows/ci.yml` | On every pull request Linux runs the checks and the two test profiles as three parallel jobs (`cargo xtask ci <gate>`); Windows runs the whole gate nightly and on dispatch; macOS is run locally | same |

Daily commands:

```sh
cargo check            # type-check only; the fast inner loop
cargo test -p <crate>  # one crate's tests
cargo clippy           # lints, as warnings
cargo fmt              # format everything
cargo doc --open       # browsable API docs for the whole workspace
cargo xtask ci         # every gate, as errors; the definition of done
```

**One step for you.** Rust here came from Homebrew, which ignores
`rust-toolchain.toml` and moves whenever `brew upgrade` runs. The standard
arrangement is rustup, which also provides `rust-analyzer` (for CLion /
RustRover) and the nightly-only tools (Miri):

```sh
brew uninstall rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # official installer; accept defaults
cargo install --locked cargo-deny                                # the optional gate
```

The installer puts everything under `~/.cargo` and `~/.rustup` and adds
`~/.cargo/bin` to your shell profile. Linux and Windows hosts use the same
installer (`rustup-init.exe` on Windows).

Until then everything works; `deny` reports itself as skipped.

### Building on the Linux validation host

Performance figures only count from the owner's private Linux validation
host ([decision 0002](../decisions/0002-performance-parity-with-cpp.md)). It
has rustup, and rustup reads `rust-toolchain.toml`, so it builds with exactly
the compiler used everywhere else. This repository is checked out in the home
directory there as `hgl`.

rustup puts `cargo` on the path from the shell's start-up files, which a
plain `ssh <host> 'cargo ...'` does not read. Run commands through a login
shell instead:

```sh
ssh <host> 'bash -lc "cd ~/hgl && git pull --ff-only && cargo xtask ci"'
```

Not installed there yet, and needed later: `cargo-deny` (the `deny` gate
reports itself as skipped until it is) and a nightly toolchain with Miri for
the store crate.

### Why the lints are what they are

Each group answers a measured habit of agent-written code
([0003](../explorations/0003-ai-codegen-evidence.md)):

- **Hidden failure** — `unwrap`, `expect`, `panic!`, `todo!`, `let _ =` on a
  `Result`, discarding an error in `map_err`. A tick path must not panic, and
  error-masking is the fastest-growing smell in the data. Tests are exempt.
- **Dodging ownership** — `.clone()` on a ref-counted pointer, redundant
  clones, `Rc<Mutex<_>>`. These are how an agent makes the borrow checker
  stop complaining without fixing the design.
- **Exhaustiveness** — no `_ =>` arm on an enum. Add a time-series kind and
  the compiler lists every place that must handle it. This is the property
  the reference's `switch`-over-`enum class` style cannot give.
- **Silencing** — `#[allow]` is banned; `#[expect(lint, reason = "...")]` is
  the only way to suppress, it must say why, and it becomes an error itself
  once the lint no longer fires.
- **`unsafe_code = "forbid"`** — not overridable from source. `unsafe` is
  allowed in exactly one crate, the store
  ([decision 0003](../decisions/0003-unsafe-confined-to-the-store.md)), which
  has its own lint table: every block carries a `// SAFETY:` comment, debug
  builds assert what release builds trust, and the crate runs under Miri.
- **Shape** — 80-line functions, nesting depth 5, six arguments. Thresholds
  are not raised to make code pass.

## Reading Rust from C++

| Rust | Read it as | The difference that matters |
|---|---|---|
| `let x = y;` | `auto x = std::move(y);` | Move is the default and the compiler rejects use-after-move. Copy only for `Copy` types (ints, floats, small ids) |
| `&T` / `&mut T` | `const T&` / `T&` | At any moment: many `&T` *or* one `&mut T`. This one rule removes aliasing bugs and is the source of most friction |
| `'a` in a signature | "the result must not outlive this argument" | A checked comment about pointer validity. Usually inferred; many explicit ones in a public API is a smell |
| `struct` + `impl` | class with non-virtual methods | No inheritance. `self`, `&self`, `&mut self` say whether a method consumes, reads or mutates |
| `enum` with data | `std::variant` that works | `match` must cover every variant |
| `trait` | a concept *and* an abstract base | `fn f<T: Tr>(x: T)` is a template (monomorphised); `&dyn Tr` / `Box<dyn Tr>` is a vtable call. Same trait, caller chooses |
| `Option<T>`, `Result<T, E>` | `std::optional`, `std::expected` | No null, no exceptions. `?` means "return the error to my caller" |
| `Box<T>`, `Rc<T>`, `Arc<T>` | `unique_ptr`, `shared_ptr` (non-atomic), `shared_ptr` | `Rc` is not thread-safe and the compiler knows it |
| `Drop` | destructor | Runs at end of scope or when moved-from owner dies; never on a moved-out value |
| `Vec<T>`, `String`, `&[T]`, `&str` | `vector`, `string`, `span`, `string_view` | Views are lifetime-checked |
| crate / `mod` / `pub` | library / namespace / `public` | Private by default, at module granularity. No headers: a crate's public surface is what `pub` reaches |
| `Send` / `Sync` | "may move to / be shared with another thread" | Compiler-derived. A type holding an `Rc` or a raw pointer is neither, so it *cannot* cross a thread |
| `#[derive(Debug, Clone, PartialEq)]` | compiler-written boilerplate | The acceptable kind of macro |
| `unsafe { }` | ordinary C++ | The compiler stops checking aliasing and lifetimes inside |

Idioms this code base will lean on, all chosen to stay readable:

- **Arenas and typed ids instead of pointers.** Things live in a `Vec` owned
  by one parent and refer to each other by a small `Copy` id
  (`struct NodeId(u32)`). This is the reference compiler's IR style already,
  it is how Rust expresses graphs without fighting the borrow checker, and a
  stale id is a detectable error instead of a dangling pointer.
- **Enums for closed sets, traits for open ones.** Value kinds and
  time-series kinds are closed: enums. Node implementations are open: a trait.
- **Newtypes.** `struct EngineTime(i64)` rather than a bare `i64`; free at
  run time, and arguments cannot be swapped.
- **Errors are values.** One error enum per crate, `?` to propagate.

## Reviewing a diff when Rust is your second language

Search the diff for these before reading it line by line:

1. `Cargo.toml` — a new dependency, or a new edge between our crates.
2. `pub` — did the public surface grow, and did it need to?
3. `clone()`, `Rc<`, `RefCell<`, `Arc<`, `Mutex<` — each is either an agreed
   part of the design or a dodge.
4. `#[expect(` — read the reason; is it one?
5. `unsafe`, `macro_rules!`, `impl<` with more than two generic parameters,
   more than one `'a` in a signature, `Box<dyn` in a hot path, `where`
   clauses longer than the function body.
6. Tests — do they assert ticks and values, or only that nothing crashed?
   A mock of our own code is a smell.
7. Line count against the crate's budget and, for a module with a card, the
   module's budget and its `uses` list in `[package.metadata.hgl.modules]`.

If a change is hard to follow, that is a finding about the change. Ask for the
plainer version; the rule in `CLAUDE.md` makes that a requirement.
