//! Ordered lexical harness checking; expression and graph selection remain frontend-owned.
use hgl_eval_data::TestStep;
use hgl_harness_ir::{Argument, Evaluation, Step};
use hgl_rust_ir::{Kind, Plan, Statement, Value};
use hgl_source::{Expr, Stmt, Ty};
use hgl_static_values::PreparedLexicalScope;
use std::collections::BTreeSet;

/// Frontend expression and graph checking in the current lexical test scope.
pub trait Check {
    /// Check an ordinary statement and preserve binding authority and cold origins.
    fn statement(
        &mut self,
        statement: &Stmt,
        scope: &mut PreparedLexicalScope,
    ) -> Result<Statement, String>;
    /// Check an ordinary boolean without executing it.
    fn boolean(&mut self, expr: &Expr, scope: &PreparedLexicalScope) -> Result<Value, String>;
    /// Select one independently instantiated graph and its written argument expressions.
    fn evaluation(
        &mut self,
        call: &hgl_eval_data::Evaluation,
        scope: &PreparedLexicalScope,
    ) -> Result<(Plan, Vec<Argument>), String>;
    /// Check expected publications against the selected graph's exact result type.
    fn expected(
        &mut self,
        ty: &Ty,
        slots: &[Option<Expr>],
        scope: &PreparedLexicalScope,
    ) -> Result<Vec<Option<Value>>, String>;
}
/// Check one lexical block, retaining only its visible bindings and executed-branch facts.
pub fn block(
    source: Vec<TestStep>,
    scope: &mut PreparedLexicalScope,
    plans: &mut Vec<Plan>,
    checker: &mut impl Check,
) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let mut names = BTreeSet::new();
    for step in source {
        let binding = match &step {
            TestStep::Ordinary(Stmt::Let(name, _, _) | Stmt::Var(name, _, _))
            | TestStep::BindEval(name, _) => Some(name),
            TestStep::Ordinary(_) | TestStep::Assert(_) | TestStep::Eval(_) | TestStep::If(..) => {
                None
            }
        };
        if let Some(name) = binding
            && !names.insert(name.clone())
        {
            return Err(format!("duplicate test local {name}"));
        }
        steps.push(match step {
            TestStep::Ordinary(statement) => Step::Ordinary(checker.statement(&statement, scope)?),
            TestStep::Assert(expr) => Step::Assert(checker.boolean(&expr, scope)?),
            TestStep::If(expr, yes, no) => {
                let condition = checker.boolean(&expr, scope)?;
                let yes = branch(yes, &condition, true, scope, plans, checker)?;
                let no = branch(no, &condition, false, scope, plans, checker)?;
                Step::If(condition, yes, no)
            }
            TestStep::Eval(call) => evaluation(&call, None, scope, plans, checker)?,
            TestStep::BindEval(name, call) => evaluation(&call, Some(name), scope, plans, checker)?,
        });
    }
    Ok(steps)
}
fn branch(
    source: Vec<TestStep>,
    condition: &Value,
    truth: bool,
    scope: &mut PreparedLexicalScope,
    plans: &mut Vec<Plan>,
    checker: &mut impl Check,
) -> Result<Vec<Step>, String> {
    let mut branch = scope.clone();
    branch.facts = hgl_flow_check::facts(condition, truth, &scope.facts);
    let steps = block(source, &mut branch, plans, checker)?;
    scope.next = branch.next;
    Ok(steps)
}
fn evaluation(
    call: &hgl_eval_data::Evaluation,
    name: Option<String>,
    scope: &mut PreparedLexicalScope,
    plans: &mut Vec<Plan>,
    checker: &mut impl Check,
) -> Result<Step, String> {
    let (plan, arguments) = checker.evaluation(call, scope)?;
    let expected = call
        .expected
        .as_ref()
        .map(|slots| {
            let (_, ty) = plan
                .output
                .as_ref()
                .ok_or("outputless eval cannot be compared")?;
            checker
                .expected(ty, slots, scope)
                .map_err(|e| format!("expected output: {e}"))
        })
        .transpose()?;
    let evaluation = Evaluation {
        case: plans.len(),
        arguments,
        expected,
    };
    let step = if let Some(name) = name {
        let (_, result) = plan
            .output
            .as_ref()
            .ok_or("outputless eval cannot be bound")?;
        let ty = Ty::List(
            Box::new(Ty::Nullable(Box::new(result.clone().delta()?))),
            None,
        );
        let id = scope.next;
        scope.next += 1;
        scope
            .bindings
            .insert(name, Value::new(ty.clone(), Kind::Local(id)));
        Step::BindEval(id, ty, evaluation)
    } else {
        Step::Eval(evaluation)
    };
    plans.push(plan);
    Ok(step)
}
