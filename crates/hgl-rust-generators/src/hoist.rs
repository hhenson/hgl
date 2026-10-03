use hgl_rust_ir::{Kind, Value};

pub(crate) fn value(source: &Value) -> Value {
    let mut result = source.clone();
    rewrite(&mut result);
    result
}
fn rewrite(value: &mut Value) {
    match &mut value.kind {
        Kind::Local(id) | Kind::MutableLocal(id) => {
            value.kind = Kind::GeneratorLocal(*id);
        }
        Kind::List(values) | Kind::Native(_, values) | Kind::Query(_, values) => {
            for value in values {
                rewrite(value);
            }
        }
        Kind::Construct(fields) => {
            for (_, value) in fields {
                rewrite(value);
            }
        }
        Kind::ValueCall(arguments, _) => {
            for value in arguments {
                rewrite(value);
            }
        }
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            rewrite(a);
            rewrite(b);
        }
        Kind::Length(value)
        | Kind::Field(value, _)
        | Kind::GlobalSet(_, value)
        | Kind::IsPresent(value)
        | Kind::Present(value)
        | Kind::Unary(_, value) => rewrite(value),
        Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
}
