//! Versioned cold mutations delegate their operation to the ordinary evaluator.
use crate::ir::{Kind, Plan, Statement, Value};
use crate::value_eval::Evaluator;
use hgl_source::Issue;

fn place(value: &Value, arguments: &mut Vec<Value>) -> Result<(Value, Value), Issue> {
    let mut target = value.clone();
    let root = if let Kind::MutableLocal(id) = &mut target.kind {
        *id = 0;
        value.clone()
    } else if let Kind::Field(parent, _) = &mut target.kind {
        let (root, rewritten) = place(parent, arguments)?;
        **parent = rewritten;
        root
    } else if let Kind::Index(parent, offset) = &mut target.kind {
        let (root, rewritten) = place(parent, arguments)?;
        **parent = rewritten;
        let id = arguments.len() + 2;
        arguments.push(*offset.clone());
        **offset = Value::new(offset.ty.clone(), Kind::Local(id));
        root
    } else {
        return Err("ordinary mutation requires writable owner access".into());
    };
    Ok((root, target))
}
/// Execute one checked mutation against an independent version of its owning root.
pub(crate) fn apply(
    evaluator: &mut Evaluator,
    plan: &mut Plan,
    target: &Value,
    input: &Value,
    push: bool,
) -> Result<(), Issue> {
    let mut offsets = Vec::new();
    let (root, mut target) = place(target, &mut offsets)?;
    let mut arguments = vec![input.clone(), root.clone()];
    arguments.extend(offsets);
    let id = arguments.len();
    crate::ir_visit::value_mut(&mut target, &mut |value| {
        if matches!(value.kind, Kind::MutableLocal(_)) {
            value.kind = Kind::MutableLocal(id);
        }
        true
    });
    let initial = Value::new(root.ty.clone(), Kind::Local(1));
    let local = Value::new(root.ty.clone(), Kind::Local(id));
    let input = Value::new(input.ty.clone(), Kind::Local(0));
    let mutation = if push {
        Statement::Call(Value::new(
            hgl_source::Ty::Void,
            Kind::Push(Box::new(target), Box::new(input)),
        ))
    } else {
        Statement::Assign(target, input)
    };
    let recipe = Value::new(
        root.ty.clone(),
        Kind::ValueCall(
            arguments,
            vec![
                Statement::Var(id, initial),
                mutation,
                Statement::Yield(local),
            ],
        ),
    );
    let updated = crate::wiring_values::value(evaluator, plan, &recipe)?;
    crate::wiring_values::replace(evaluator, plan, &root, updated)
}
