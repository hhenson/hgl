# Card: hgl-source

Lexer and body syntax shared by library indexing and graph checking. Uses only
`hgl-calendar`, `hgl-type-shape` and `hgl-lex`; budget 700 lines. No runtime dependency.

Surface: `Token { text, span }`, `lex(&str) -> Result<Vec<Token>, String>`;
`Ty` and `Ty::{parse,name}`; `Literal` and `Literal::ty`; `Expr` and
`Expr::{fixed,handler_guard}`; `Stmt`; `Cursor { tokens, pos }` with `new`, `peek`, `at`,
`take`, `need`, `consume`, `lines`, `name`, `type_name`, `expr`, `block`.
The enums describe literals, names, calls, unary/binary expressions, dense
sequences, lets, returns, assignments, calls, conditionals and element loops.
Spans and documentation tokens retain source text for diagnostics/emission.

Calendar literals are validated; fixed duration arithmetic is checked. Explicit
`ref<ref<T>>` is rejected. Aliases and full recursive type checking belong to
the compiler, not this parser. `_` is admitted only as a dense sequence cell.

Acceptance: shared library parsing and the compiler's negative/type tests;
calendar boundary tests exercise literal conversion separately.

`Expr::handler_guard(self, inputs: &[String]) -> Expr` normalizes a temporal
handler predicate: contextual empty valid/modified selectors expand to the
supplied input names; missing top-level conjunction selectors are prepended.
Selectors under disjunction/negation/residual calls do not suppress defaults.
This pure AST transform is independent of types and runtime representation;
backends choose their admitted handler profile before applying it.

`Expr::{Null,Index}` represents contextual absence and postfix indexing;
`Stmt::{Var,Exit}` distinguishes mutable locals and a bare runtime return from
immutable `Let` and value-return statements. `Ty::Nullable` is an internal
inferred expression type, never a source annotation accepted by `Ty::parse`.
These forms add no general optional type or null harness-cell alias. Source
checking determines which indexed receivers and nullable uses are admitted.

`Expr::Property(Box<Expr>, String)` represents ordinary postfix property/field
selection, separately from `::` qualified names. It is not a method call.
`Stmt::{Assign,Add}` retain an expression target for checking: the parser does
not treat a dotted target as an ordinary variable name. The checker determines
whether the selected field is writable; no new writable field domain is admitted.

`Stmt::{Let,Var}` carry an optional source type annotation alongside their
name and initializer. Annotation resolution and initializer compatibility are
frontend checks; a typed local supplies ordinary expected-value context.

`Ty::Struct(Nominal, Vec<(String, Ty)>)` is a resolved finite ordinary struct:
qualified nominal identity plus declared field names and types. It is created
by the frontend, not `Ty::parse`; optional/recursive schemas remain
outside this executable subset.

`Ty::List(element, fixed_size)` preserves ordinary element type and optional
exact size. The parser accepts explicit `unbounded` as the same identity as
omitted size; numeric sizes fit nonnegative i64. `Ty::list_parts` splits the
outer list arguments without losing nested list/struct spellings. Existing
`Expr::Sequence` and `Expr::Index` are contextual ordinary list syntax as well
as their separate harness/replay uses; admission belongs to the checker.

Ty and Nominal are re-exported from hgl-type-shape. `Expr::Applied` retains an
explicit struct constructor application separately from ordinary calls. Generic
argument lookahead follows the pinned grammar before resolving the callee;
comparison parsing remains independent of visible declarations. Source type
spelling accepts nested multiple generic arguments. Checking rejects unsupported
argument roles and explicit generic callable application.

Token and lex are re-exported from hgl-lex. `Stmt::TimedYield(time,payload)`
retains ordered generator operands. `Stmt::While(condition,body)` retains an
omitted condition as true. `yield` and `while` are reserved identifiers.
`Cursor::block_contents` parses statements after an already-consumed opening
brace, for function headers that contain injections. Phase and generator
admission remain checker responsibilities.
