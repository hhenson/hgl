//! Ordered lexical harness checking; expression and graph selection remain frontend-owned.
use crate::eval_data::TestStep;
use crate::harness_ir::{Argument, Evaluation, Step};
use crate::ir::{Kind, Plan, Statement, Value};
use crate::static_values::PreparedLexicalScope;
use hgl_source::{Expr, Issue, Stmt, Ty};
use std::collections::BTreeSet;

/// Frontend expression and graph checking in the current lexical test scope.
pub trait Check {
    /// Check an ordinary statement and preserve binding authority and cold origins.
    fn statement(
        &mut self,
        statement: &Stmt,
        scope: &mut PreparedLexicalScope,
    ) -> Result<Statement, Issue>;
    /// Check an ordinary boolean without executing it.
    fn boolean(&mut self, expr: &Expr, scope: &PreparedLexicalScope) -> Result<Value, Issue>;
    /// Select one independently instantiated graph and its written argument expressions.
    fn evaluation(
        &mut self,
        call: &crate::eval_data::Evaluation,
        scope: &PreparedLexicalScope,
    ) -> Result<(Plan, Vec<Argument>), Issue>;
    /// Check expected publications against the selected graph's exact result type.
    fn expected(
        &mut self,
        ty: &Ty,
        slots: &[Option<Expr>],
        scope: &PreparedLexicalScope,
    ) -> Result<Vec<Option<Value>>, Issue>;
}
/// Check one lexical block, retaining only its visible bindings and executed-branch facts.
pub fn block(
    source: Vec<TestStep>,
    scope: &mut PreparedLexicalScope,
    plans: &mut Vec<Plan>,
    checker: &mut impl Check,
) -> Result<Vec<Step>, Issue> {
    let mut steps = Vec::new();
    let mut names = BTreeSet::new();
    for step in source {
        let binding = match &step {
            TestStep::Ordinary(Stmt::Let(name, _, _) | Stmt::Var(name, _, _))
            | TestStep::BindEval(name, _) => Some(name),
            TestStep::Raises(..)
            | TestStep::Ordinary(_)
            | TestStep::Assert(_)
            | TestStep::Eval(_)
            | TestStep::If(..) => None,
        };
        if let Some(name) = binding
            && !names.insert(name.clone())
        {
            return Err(format!("duplicate test local {name}").into());
        }
        steps.push(match step {
            TestStep::Raises(code, source) => {
                let mut local = scope.clone();
                let body = block(source, &mut local, plans, checker)?;
                scope.next = local.next;
                Step::Raises(code, body)
            }
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
) -> Result<Vec<Step>, Issue> {
    let mut branch = scope.clone();
    branch.facts = crate::flow_check::facts(condition, truth, &scope.facts);
    let steps = block(source, &mut branch, plans, checker)?;
    scope.next = branch.next;
    Ok(steps)
}
fn evaluation(
    call: &crate::eval_data::Evaluation,
    name: Option<String>,
    scope: &mut PreparedLexicalScope,
    plans: &mut Vec<Plan>,
    checker: &mut impl Check,
) -> Result<Step, Issue> {
    let (plan, arguments) = checker.evaluation(call, scope)?;
    let expected = call
        .expected
        .as_ref()
        .map(|slots| {
            let (_, ty) = plan
                .output
                .as_ref()
                .ok_or("outputless eval cannot be compared")?;
            checker.expected(ty, slots, scope).map_err(|mut issue| {
                issue.message = format!("expected output: {}", issue.message);
                issue
            })
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
