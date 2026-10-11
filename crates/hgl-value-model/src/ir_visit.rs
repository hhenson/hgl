//! Syntax-neutral checked IR traversal with explicit subtree pruning.
use crate::ir::{Kind, Node, Statement, Value};
/// Visit one expression before its descendants; false preserves a selected complete boundary.
pub fn value<'a>(input: &'a Value, visit: &mut impl FnMut(&'a Value) -> bool) {
    if !visit(input) {
        return;
    }
    match &input.kind {
        Kind::Delta(parts) => {
            for part in parts {
                for input in part.operands() {
                    value(input, visit);
                }
            }
        }
        Kind::List(items) | Kind::Native(_, items) | Kind::Query(_, items) => {
            for input in items {
                value(input, visit);
            }
        }
        Kind::Construct(fields) | Kind::Captured(_, fields) => {
            for (_, input) in fields {
                value(input, visit);
            }
        }
        Kind::ValueCall(args, body) => {
            for input in args {
                value(input, visit);
            }
            statements(body, visit);
        }
        Kind::Field(input, _)
        | Kind::GlobalSet(_, input)
        | Kind::Length(input)
        | Kind::Unary(_, input)
        | Kind::IsPresent(input)
        | Kind::Present(input) => value(input, visit),
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            value(a, visit);
            value(b, visit);
        }
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::IterationInput(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
}
/// Visit statement operands in source order, including nested ordinary helper bodies.
pub fn statements<'a>(body: &'a [Statement], visit: &mut impl FnMut(&'a Value) -> bool) {
    for statement in body {
        match statement {
            Statement::Let(_, v)
            | Statement::Var(_, v)
            | Statement::Borrow(_, v, _)
            | Statement::Return(v)
            | Statement::Yield(v)
            | Statement::Call(v) => value(v, visit),
            Statement::TimedYield(a, b) | Statement::Assign(a, b) => {
                value(a, visit);
                value(b, visit);
            }
            Statement::While(v, body)
            | Statement::ForItems(_, _, _, v, body)
            | Statement::For(_, v, body) => {
                value(v, visit);
                statements(body, visit);
            }
            Statement::If(v, a, b) => {
                value(v, visit);
                statements(a, visit);
                statements(b, visit);
            }
            Statement::Exit => {}
        }
    }
}
/// Visit hook expressions; configurations keep their own cold preparation boundary.
pub fn hooks<'a>(node: &'a Node, visit: &mut impl FnMut(&'a Value) -> bool) {
    statements(&node.start, visit);
    statements(&node.stop, visit);
    if let Some(body) = &node.generator {
        statements(body, visit);
    }
    for (guard, body) in &node.handlers {
        if let Some(guard) = guard {
            value(guard, visit);
        }
        statements(body, visit);
    }
}

/// Rewrite every hook operand through the same compiled-node boundaries.
pub fn hooks_mut(node: &mut Node, visit: &mut impl FnMut(&mut Value) -> bool) {
    statements_mut(&mut node.start, visit);
    statements_mut(&mut node.stop, visit);
    if let Some(body) = &mut node.generator {
        statements_mut(body, visit);
    }
    for (guard, body) in &mut node.handlers {
        if let Some(guard) = guard {
            value_mut(guard, visit);
        }
        statements_mut(body, visit);
    }
}

/// Rewrite checked expression metadata before visiting its retained descendants.
pub fn value_mut(input: &mut Value, visit: &mut impl FnMut(&mut Value) -> bool) {
    if !visit(input) {
        return;
    }
    match &mut input.kind {
        Kind::List(items) | Kind::Native(_, items) | Kind::Query(_, items) => {
            for input in items {
                value_mut(input, visit);
            }
        }
        Kind::Construct(fields) | Kind::Captured(_, fields) => {
            for (_, input) in fields {
                value_mut(input, visit);
            }
        }
        Kind::Delta(parts) => {
            for part in parts {
                for input in part.operands_mut() {
                    value_mut(input, visit);
                }
            }
        }
        Kind::ValueCall(args, body) => {
            for input in args {
                value_mut(input, visit);
            }
            statements_mut(body, visit);
        }
        Kind::Field(v, _)
        | Kind::GlobalSet(_, v)
        | Kind::Length(v)
        | Kind::Unary(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v) => value_mut(v, visit),
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            value_mut(a, visit);
            value_mut(b, visit);
        }
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::IterationInput(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
}
/// Rewrite checked operands in source order without evaluating them.
pub fn statements_mut(body: &mut [Statement], visit: &mut impl FnMut(&mut Value) -> bool) {
    for statement in body {
        match statement {
            Statement::Let(_, v)
            | Statement::Var(_, v)
            | Statement::Borrow(_, v, _)
            | Statement::Return(v)
            | Statement::Yield(v)
            | Statement::Call(v) => value_mut(v, visit),
            Statement::TimedYield(a, b) | Statement::Assign(a, b) => {
                value_mut(a, visit);
                value_mut(b, visit);
            }
            Statement::While(v, body)
            | Statement::ForItems(_, _, _, v, body)
            | Statement::For(_, v, body) => {
                value_mut(v, visit);
                statements_mut(body, visit);
            }
            Statement::If(v, a, b) => {
                value_mut(v, visit);
                statements_mut(a, visit);
                statements_mut(b, visit);
            }
            Statement::Exit => {}
        }
    }
}

/// Inspect lexical statements independently of ordinary helper-local scopes.
pub fn nodes<'a>(body: &'a [Statement], visit: &mut impl FnMut(&'a Statement)) {
    for statement in body {
        visit(statement);
        if let Statement::If(_, a, b) = statement {
            nodes(a, visit);
            nodes(b, visit);
        } else if let Statement::While(_, body)
        | Statement::For(_, _, body)
        | Statement::ForItems(_, _, _, _, body) = statement
        {
            nodes(body, visit);
        }
    }
}

/// Rewrite lexical statements independently of expression execution or helper-local scopes.
pub fn nodes_mut(body: &mut [Statement], visit: &mut impl FnMut(&mut Statement)) {
    for statement in body {
        visit(statement);
        match statement {
            Statement::If(_, a, b) => {
                nodes_mut(a, visit);
                nodes_mut(b, visit);
            }
            Statement::While(_, body)
            | Statement::For(_, _, body)
            | Statement::ForItems(_, _, _, _, body) => nodes_mut(body, visit),
            Statement::Exit
            | Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..) => {}
        }
    }
}
