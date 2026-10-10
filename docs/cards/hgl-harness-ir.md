# Card: hgl-harness-ir

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `harness_ir` module of `hgl-semantics` (`crates/hgl-semantics/src/harness_ir.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Checked ordered test preparation; uses hgl-rust-ir and hgl-source. Budget 200
source lines. Test { name: String, steps: Vec<Step> } preserves lexical order.
Step::{Ordinary(Statement), Assert(Value), Eval(Evaluation)} distinguishes setup,
ordinary assertions and graph execution. Evaluation contains case: usize,
arguments: Vec<Argument>, expected: Option<Vec<Option<Value>>>. Arguments remain
in written order: Constant { binding: usize, value: Value } or Dense { binding:
usize, parameter: String, shape: Ty, entry_type: Ty, slots: Vec<Option<Value>> }. PreparedEval contains independently
owned arguments: Vec<Value> in binding order and input_length: usize.

The generated graph plan is separate. Binding indices select compile-time known
configuration positions; no graph topology or type is selected from provider
results. Expected expressions execute after the eval expression. This adds no
source test grammar; flag missing checked forms before adding APIs.

Dense entry_type is the checked declaration-owned TimedValue<T> type. Use it
with existing eval-data::timed construction; never reconstruct nominal identity
from a spelling or discard the originating temporal type.

CapturedEval { length: usize, ticks: Vec<(usize, Value)> } owns sparse captured
publications and the dense logical horizon. Silent gaps never allocate cells.

Dense parameter retains the declared parameter name for existing named-input
and zero-based position diagnostics; it does not select a runtime type.

Step::BindEval(binding, sequence_type, Evaluation) retains the owned returned
horizon/publications under an immutable local; sequence_type is the existing
contextual List<Nullable<payload>> checker representation. Step::If(condition,
yes,no) contains lexical step blocks. Neither form changes graph composition.

Step::Raises(code, body) owns a lexical assertion block. Failure distinguishes Execution(NodeError) from Other(String); matches excludes cleanup-only and cleanup-failed execution. Uses hgl-node-error.

Suite { tests, plans } owns the checked test inventory and graph plans. select accepts short/qualified names, rejects each unknown name, and filters tests without renumbering graph plan indices.

Suite is Default. require_tests preserves the ordinary empty-suite error after mixed-run orchestration.

Argument::Dense retains optional boxed ordinary sequence IR alongside literal sparse slots. The harness evaluates that sequence once, then applies the same fresh pre-start trace validation.

From<EvalError> for Failure preserves coded ordinary execution failures as NodeError; other ordinary failures keep their existing text channel. May use value_eval. Cleanup and phase matching rules remain unchanged.
