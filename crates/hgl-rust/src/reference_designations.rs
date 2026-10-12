//! Checked publication flow supplies finite routes without observing target payloads.
use crate::ir::{Kind, Node, Statement, Value};
use std::collections::{BTreeMap, BTreeSet};

type Routes = BTreeSet<(usize, Vec<usize>)>;
type Locals = BTreeMap<usize, Routes>;

fn value(value: &Value, locals: &Locals, output: &Routes) -> Routes {
    match &value.kind {
        Kind::Input(input, false) => BTreeSet::from([(*input, Vec::new())]),
        Kind::Local(id) | Kind::MutableLocal(id) | Kind::GeneratorLocal(id) => {
            locals.get(id).cloned().unwrap_or_default()
        }
        Kind::Output => output.clone(),
        Kind::Field(parent, index) => project(self::value(parent, locals, output), *index),
        Kind::Query(op, args) if op == "\0reference" => self::value(&args[0], locals, output),
        Kind::ValueCall(args, body) => {
            let mut inner = args
                .iter()
                .enumerate()
                .map(|(id, arg)| (id, self::value(arg, locals, output)))
                .collect();
            let mut returned = Routes::new();
            statements(body, &mut inner, output, &mut returned);
            returned
        }
        Kind::Opaque(_)
        | Kind::IterationInput(_)
        | Kind::Captured(..)
        | Kind::Delta(_)
        | Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::List(_)
        | Kind::Index(..)
        | Kind::Length(_)
        | Kind::Push(..)
        | Kind::Configuration(_)
        | Kind::Construct(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::GlobalSet(..)
        | Kind::IsPresent(_)
        | Kind::Present(_)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::Native(..)
        | Kind::Binary(..)
        | Kind::Unary(..)
        | Kind::Query(..)
        | Kind::Capability
        | Kind::Void => Routes::new(),
    }
}

fn project(routes: Routes, index: usize) -> Routes {
    routes
        .into_iter()
        .map(|(input, mut path)| {
            path.push(index);
            (input, path)
        })
        .collect()
}

fn merge(locals: &mut Locals, other: Locals) {
    for (id, routes) in other {
        locals.entry(id).or_default().extend(routes);
    }
}

fn statements(
    body: &[Statement],
    locals: &mut Locals,
    output: &Routes,
    returned: &mut Routes,
) -> bool {
    for statement in body {
        match statement {
            Statement::Let(id, source)
            | Statement::Var(id, source)
            | Statement::Borrow(id, source, _) => {
                locals.insert(*id, value(source, locals, output));
            }
            Statement::Assign(target, source) => {
                let routes = value(source, locals, output);
                if let Kind::MutableLocal(id) | Kind::GeneratorLocal(id) = target.kind {
                    locals.insert(id, routes);
                } else if matches!(target.kind, Kind::Output) {
                    returned.extend(routes);
                }
            }
            Statement::Return(source) | Statement::Yield(source) => {
                returned.extend(value(source, locals, output));
                return false;
            }
            Statement::Exit => return false,
            Statement::If(_, yes, no) => {
                let mut yes_locals = locals.clone();
                let mut no_locals = locals.clone();
                let yes_live = statements(yes, &mut yes_locals, output, returned);
                let no_live = statements(no, &mut no_locals, output, returned);
                match (yes_live, no_live) {
                    (true, true) => {
                        *locals = yes_locals;
                        merge(locals, no_locals);
                    }
                    (true, false) => *locals = yes_locals,
                    (false, true) => *locals = no_locals,
                    (false, false) => return false,
                }
            }
            Statement::While(_, body)
            | Statement::For(_, _, body)
            | Statement::ForItems(_, _, _, _, body) => loop {
                let mut next = locals.clone();
                let live = statements(body, &mut next, output, returned);
                if live {
                    let previous = locals.clone();
                    merge(locals, next);
                    if *locals != previous {
                        continue;
                    }
                }
                break;
            },
            Statement::TimedYield(_, source) => returned.extend(value(source, locals, output)),
            Statement::Call(_) => {}
        }
    }
    true
}

pub(crate) fn inputs(node: &Node) -> Routes {
    let mut output = Routes::new();
    loop {
        let mut returned = Routes::new();
        for body in std::iter::once(&node.start)
            .chain(node.handlers.iter().map(|(_, body)| body))
            .chain(node.generator.iter())
        {
            statements(body, &mut Locals::new(), &output, &mut returned);
        }
        let previous = output.clone();
        output.extend(returned);
        if output == previous {
            return output;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Locals, Routes, statements, value};
    use crate::ir::{Kind, Statement, Value};
    use hgl_source::Ty;
    use std::collections::BTreeSet;

    fn reference(kind: Kind) -> Value {
        Value::new(Ty::Ref(Box::new(Ty::I64)), kind)
    }

    #[test]
    fn local_replacement_excludes_old_and_metadata_designations() {
        let body = [
            Statement::Var(0, reference(Kind::Input(0, false))),
            Statement::Let(
                1,
                Value::new(
                    Ty::Bool,
                    Kind::Query("valid".into(), vec![reference(Kind::Input(2, false))]),
                ),
            ),
            Statement::Assign(
                reference(Kind::MutableLocal(0)),
                reference(Kind::Input(1, false)),
            ),
            Statement::Return(reference(Kind::MutableLocal(0))),
        ];
        let mut routes = Routes::new();
        statements(&body, &mut Locals::new(), &Routes::new(), &mut routes);
        assert_eq!(routes, BTreeSet::from([(1, Vec::new())]));
    }

    #[test]
    fn terminating_branch_does_not_leak_local_state_into_the_other_return() {
        let body = [
            Statement::Var(0, reference(Kind::Input(0, false))),
            Statement::If(
                Value::new(Ty::Bool, Kind::Input(3, false)),
                vec![
                    Statement::Assign(
                        reference(Kind::MutableLocal(0)),
                        reference(Kind::Input(1, false)),
                    ),
                    Statement::Exit,
                ],
                Vec::new(),
            ),
            Statement::Return(reference(Kind::MutableLocal(0))),
        ];
        let mut routes = Routes::new();
        statements(&body, &mut Locals::new(), &Routes::new(), &mut routes);
        assert_eq!(routes, BTreeSet::from([(0, Vec::new())]));
    }

    #[test]
    fn helper_yield_and_held_output_preserve_projection_paths() {
        let projected = reference(Kind::Query(
            "\0reference".into(),
            vec![Value::new(
                Ty::I64,
                Kind::Field(
                    Box::new(Value::new(
                        Ty::Tuple(vec![Ty::I64, Ty::I64]),
                        Kind::Input(2, false),
                    )),
                    1,
                ),
            )],
        ));
        let call = reference(Kind::ValueCall(
            vec![projected, reference(Kind::Input(3, false))],
            vec![Statement::Yield(reference(Kind::Local(0)))],
        ));
        let held = BTreeSet::from([(4, vec![0])]);
        assert_eq!(
            value(&call, &Locals::new(), &held),
            BTreeSet::from([(2, vec![1])])
        );
        assert_eq!(value(&reference(Kind::Output), &Locals::new(), &held), held);
    }
}
