# Card: hgl-local-check

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `local_check` module of `hgl-semantics` (`crates/hgl-semantics/src/local_check.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Check initialized local categories and ordinary mutations, preserving existing
ownership, observation and arithmetic rules. Uses hgl-source, hgl-rust-ir,
hgl-value-check and hgl-endpoint-check. Budget: 220 source lines. No runtime or
third-party dependencies.

Public surface:

- `initializer(Value, Option<&Ty>) -> Result<Value, String>` checks a resolved
  annotation without changing category. Ordinary i64 values widen to f64;
  temporal connections retain exact shape. An annotation cannot create a port.
- `statement(usize, Value, bool, bool) -> Result<(Value, Statement), String>`
  creates an ordinary local and checked initialization, delegating ownership
  and lexical borrow authority to hgl-value-check.
- `assignment_type(&Value, bool) -> Result<Ty, String>` checks writable
  target authority and resolves output publication context; the boolean
  indicates evaluation phase.
- `assignment(Value, Value) -> Result<Statement, String>` checks ordinary
  replacement and rejects a temporal RHS; caller checks target authority and
  phase. Local widening does not broaden cache/output publication rules.
- `replacement(&Value, Value, bool) -> Result<Value, String>` checks graph
  local write authority, exact connection type and fixed initializer category.
  The boolean identifies a mutable connection local, never a parameter or let.
- `binary(&str, [Value; 2], &mut Plan) -> Result<Value, String>` uses the existing
  hgl-value-check binary type table and checked scalar expression IR. With a
  temporal operand, converts ports to node inputs and prepared ordinary values
  to configuration, then emits a normal node with the existing bare-handler
  validity/modification guard. The caller evaluates other ordinary wiring
  operands once before construction. No new arithmetic or overload rules.

Contract: pinned contextual-local-bindings specification. Initialization fixes
category and type; compound assignment checks its result; immutable aliases and
previously consumed connections retain their original targets. Dynamic graph
conditionals and uninitialized declarations remain outside the frontend subset.
Ordinary construction and node execution remain separate. No per-tick name
lookup, allocation, dynamic category check or graph mutation is introduced.

Acceptance: pinned positive example and initialized-local fixtures, source
checking of category/type/let violations including unused and nested writes,
executed alias/consumed-port/scalar snapshots, scalar widening, and temporal
compound results. Existing ownership, borrow and runtime-local tests remain.
Mutants: allow cross-category replacement, forget mutable-port authority, or
skip fixed type checking; each must fail its targeted source checking test.

`GraphLocals` is defaultable, cloneable lexical signal access metadata. `new` consumes an
iterator of formal signal names; `bind(&mut self, &str, &Expr)` propagates direct
aliases; `payload(&self, (&Expr, &Expr))` rejects signal payload arithmetic;
`replacement(&self, &str, &Expr)` diagnoses unsupported signal rebinding. These
methods return `Result<(), String>` except construction/bind. Callers restore
metadata after nested composition calls. This does not claim complete signal
provenance through forwarded calls.
`graph_result(Value, &Ty) -> Result<Value, String>` retains the existing
composition result category/type boundary, including REF compatibility and void.

Ordinary collection equality does not read structural temporal endpoints. Binary
checking rejects set/map endpoint operands before scalar lowering; complete
ordinary constructors, configuration and explicit atomic observations retain
their existing value category.

Supported current Tuple inputs initialize independently retained ordinary locals,
using tuple_values classification. Mutable snapshot locals and snapshot aggregate
comparisons are explicitly outside this implementation profile. Complete native
ordinary tuples retain the existing writable ordinary value behavior.

Own-output assignment delegates complete ordinary/delta compatibility to
structural_admission. Writable ordinary local widening remains separate from
exact temporal output compatibility. Retained concrete Struct/fixed List/Map
locals share Tuple ownership rules; mutable retained locals remain diagnosed.
