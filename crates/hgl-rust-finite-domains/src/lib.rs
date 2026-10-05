//! Cold reachable publication domains derived from checked finite source recipes.
mod constants;
mod topology;
pub use constants::collect as constants;
pub use topology::{DECLARATION, include};

/// Maximum syntactic membership mutations in a single handler traversal.
/// Set-element loops inherit their source's publication-width bound in capacity planning.
pub fn mutations(body: &[hgl_rust_ir::Statement]) -> usize {
    use hgl_rust_ir::{Kind, Statement, Value};
    body.iter()
        .map(|statement| match statement {
            Statement::Call(Value {
                kind: Kind::Query(op, _),
                ..
            }) if op == "set_upsert" || op == "set_discard" => 1,
            Statement::If(_, yes, no) => mutations(yes).max(mutations(no)),
            Statement::For(_, _, body) | Statement::While(_, body) => mutations(body),
            Statement::Exit
            | Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..) => 0,
        })
        .sum()
}

mod widths;
pub use widths::widths;
