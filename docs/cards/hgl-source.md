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

Contextual `delta<type>` is accepted in type positions without reserving
its value-level name; the same marker followed by arguments is a constructor.
No parenthesized type-marker alias is accepted. `Expr::Sparse` retains ordered
constant-key/payload syntax only inside delta constructor arguments; ordinary list and harness
sequence parsing does not admit sparse entries. Constructor shape, names,
constant positions and payload compatibility remain checker obligations.

`value_type(&str) -> bool` checks source value-type grammar before type
normalization. Value containers recurse through value types; named generic
arguments and delta origins use full type grammar. `Cursor::type_name` validates
atomic payload value-type syntax recursively before scalar normalization.

`Expr::Tuple` preserves contextual positional values and harness omissions.
Grouping has no comma; a one-element tuple has a trailing comma. Source checking
distinguishes complete ordinary tuples from sparse temporal tuple publications;
omissions are admitted only in the latter context.

## Temporal scalar preparation

Literal and numeric conversion move to hgl-literals and remain re-exported.
TemporalLiteral is also re-exported for checked IR and preparation clients.
May use hgl-literals. Expr::TemporalLiteral(TemporalLiteral) preserves unresolved
provider-dependent construction. Expr::fixed never treats it as a closed value.
The lexer retains a complete bracketed zone annotation in one temporal token.

Re-export ParsedLiteral with Literal and TemporalLiteral for schema defaults.

Reexports RecursiveType and NominalDefinition alongside Ty and Nominal. Recursive
batch formation belongs to semantic resolution; syntax introduces no new type.

Reexports FamilyType with the other exact checked nominal carriers.

Typed map constructors accept ordered expression-colon-expression entries only within their argument lists. Existing untyped list and harness grammars remain unchanged; exact items-only admission belongs to hgl-collection-check.

Parser Cursor methods return hgl-diagnostics::Issue with original token ranges.
need and unexpected EOF emit syntax.expected_token at the required-token
origin; other parser errors retain uncoded identity. Issue is reexported and
Cursor::span exposes the current token or original EOF range. Existing String
callers retain a conversion boundary. May depend on hgl-diagnostics.

Stmt::TimedYield retains its time operand primary Range<usize> as a third field
so resolved semantic errors keep the original source location after parsing.

Expr::Located retains expression and optional named argument origins; syntax, span and argument_span expose them without changing expression identity. Typed arrow lambda expressions are parsed for source checking; valid callable lowering remains outside the executable backend profile. Paired items iteration retains both declared local names.
