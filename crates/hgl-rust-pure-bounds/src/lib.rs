//! Cold bounds for checked pure expressions, without invoking source hooks.
use hgl_rust_ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;
#[derive(Clone)]
enum Text {
    Width(usize),
    Fields(BTreeMap<usize, Self>),
}
impl Text {
    fn merge(&mut self, other: Self) {
        match (self, other) {
            (Self::Width(left), Self::Width(right)) => *left = (*left).max(right),
            (Self::Fields(left), Self::Fields(right)) => {
                for (id, bound) in right {
                    left.entry(id).or_insert(Self::Width(1)).merge(bound);
                }
            }
            (Self::Fields(fields), Self::Width(width)) => {
                for bound in fields.values_mut() {
                    bound.merge(Self::Width(width));
                }
            }
            (left @ Self::Width(_), mut right @ Self::Fields(_)) => {
                right.merge(left.clone());
                *left = right;
            }
        }
    }
    fn width(&self) -> usize {
        match self {
            Self::Width(width) => *width,
            Self::Fields(fields) => fields.values().map(Self::width).max().unwrap_or(1),
        }
    }
}
fn text(value: &Value, locals: &BTreeMap<usize, Text>) -> Text {
    match &value.kind {
        Kind::Local(id) | Kind::MutableLocal(id) | Kind::GeneratorLocal(id) => {
            locals.get(id).cloned().unwrap_or(Text::Width(1))
        }
        Kind::Construct(fields) => Text::Fields(
            fields
                .iter()
                .map(|(id, value)| (*id, text(value, locals)))
                .collect(),
        ),
        Kind::Field(value, id) => match text(value, locals) {
            Text::Fields(fields) => fields.get(id).cloned().unwrap_or(Text::Width(1)),
            other @ Text::Width(_) => other,
        },
        Kind::Binary(op, a, b) if value.ty == Ty::Str && op == "+" => Text::Width(
            text(a, locals)
                .width()
                .saturating_add(text(b, locals).width()),
        ),
        Kind::Delta(_)
        | Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::List(_)
        | Kind::Index(..)
        | Kind::Length(_)
        | Kind::Push(..)
        | Kind::ValueCall(..)
        | Kind::Configuration(_)
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
        | Kind::Output
        | Kind::Capability
        | Kind::Void => Text::Width(1),
    }
}
fn assign(target: &Value, value: Text, locals: &mut BTreeMap<usize, Text>) {
    if let Kind::MutableLocal(id) | Kind::GeneratorLocal(id) = target.kind {
        locals.insert(id, value);
    } else if let Kind::Field(parent, id) = &target.kind {
        let mut fields = match text(parent, locals) {
            Text::Fields(fields) => fields,
            Text::Width(_) => BTreeMap::new(),
        };
        fields.insert(*id, value);
        assign(parent, Text::Fields(fields), locals);
    }
}
fn text_statements(body: &[Statement], locals: &mut BTreeMap<usize, Text>, maximum: &mut usize) {
    for statement in body {
        match statement {
            Statement::Let(id, value) | Statement::Var(id, value) => {
                let bound = text(value, locals);
                *maximum = (*maximum).max(bound.width());
                locals.insert(*id, bound);
            }
            Statement::Assign(target, value) => {
                let bound = text(value, locals);
                *maximum = (*maximum).max(bound.width());
                assign(target, bound, locals);
            }
            Statement::Return(value)
            | Statement::Yield(value)
            | Statement::Call(value)
            | Statement::TimedYield(_, value) => {
                *maximum = (*maximum).max(text(value, locals).width());
            }
            Statement::If(_, yes, no) => {
                let mut left = locals.clone();
                let mut right = locals.clone();
                text_statements(yes, &mut left, maximum);
                text_statements(no, &mut right, maximum);
                for (id, bound) in left.into_iter().chain(right) {
                    locals.entry(id).or_insert(Text::Width(1)).merge(bound);
                }
            }
            Statement::For(_, _, body) | Statement::While(_, body) => {
                text_statements(body, &mut locals.clone(), maximum);
            }
            Statement::Borrow(..) | Statement::Exit => {}
        }
    }
}
fn node_factor(plan: &Plan, index: usize, factors: &mut [Option<usize>]) -> usize {
    if let Some(factor) = factors[index] {
        return factor;
    }
    let node = &plan.nodes[index];
    let input = node
        .inputs
        .iter()
        .map(|(_, source, _)| node_factor(plan, *source, factors))
        .max()
        .unwrap_or(1);
    let mut maximum = 1;
    text_statements(&node.start, &mut BTreeMap::new(), &mut maximum);
    for (_, body) in &node.handlers {
        text_statements(body, &mut BTreeMap::new(), &mut maximum);
    }
    let factor = input.saturating_mul(maximum);
    factors[index] = Some(factor);
    factor
}
/// Byte multiplier along real dependencies for a plan with a separately established finite proof.
pub fn text_factor(plan: &Plan) -> usize {
    let mut factors = vec![None; plan.nodes.len()];
    (0..plan.nodes.len())
        .map(|index| node_factor(plan, index, &mut factors))
        .max()
        .unwrap_or(1)
}
