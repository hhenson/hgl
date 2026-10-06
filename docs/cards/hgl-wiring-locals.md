# Card: hgl-wiring-locals

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `wiring_locals` module of `hgl-semantics` (`crates/hgl-semantics/src/wiring_locals.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Check and select ordinary wiring-time conditional blocks that rebind existing
connections. Uses hgl-source, hgl-rust-ir, hgl-value-eval, hgl-value-access,
hgl-value-check and hgl-local-check. Budget: 300 source lines. No runtime or
third-party dependencies.

Public surface:

- `handles(&Stmt, &BTreeMap<String, Value>) -> bool` identifies conditional
  assignments to existing temporal names. Ordinary-only conditionals retain
  the existing frontend path.
- `conditional(&Stmt, &mut BTreeMap<String, Value>, &BTreeSet<String>,
  (&GraphLocals, &StaticValues), (&mut Plan, &mut Evaluator), &mut usize)
  -> Result<(), String>` checks both branches, selects a fixed ordinary bool,
  and retains the selected bindings, ordinary evaluator state and generated
  binary nodes. The set contains mutable temporal names; the final argument
  allocates ordinary local identities.

The admitted branch profile consists of lexical let/var declarations, name
assignment, compound addition and nested ordinary if blocks. Operands are
existing local values/connections, scalar literals, checked binary expressions
and ordinary boolean negation. Annotations must resolve as concrete built-in
types. Calls and other unsupported forms produce explicit diagnostics; this is
not general graph control-flow lowering. Provider-dependent or temporal
conditions cannot select topology. StaticValues resolves only already known
ordinary preparation values; no provider value is fabricated or executed.

Every branch checks category/type/let authority, including unselected writes.
Each branch has independent ordinary evaluator state. Unselected bindings,
construction failures and nodes are discarded before graph execution. Calls
and external effects cannot enter this checker. Existing arithmetic typing,
numeric widening, ownership and evaluation are delegated to their owning
crates. The selected path preserves prior aliases and consumed ports.

Lexical shadowing does not inherit outer write authority. Assignments to an
outer binding before a later same-name shadow still reach that outer binding.
Same-block duplicate declarations are rejected; branch-local names never escape.

Acceptance: executed fixed true/false configuration, nested compound rebinds,
scalar writes mixed with port writes, alias snapshots, branch shadows, writes
before shadows, and discarded runtime-failing nodes. Checking covers inactive
category/type/let violations, duplicate declarations, escaping names and
unsupported calls. Existing dynamic-condition and ordinary-local tests remain.
Mutant: choose the wrong branch; executed fixed-condition expectations must fail.
