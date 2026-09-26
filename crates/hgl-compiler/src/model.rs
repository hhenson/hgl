use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parameter {
    pub name: String,
    pub constant: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct Function {
    pub name: String,
    pub source: usize,
    pub span: Range<usize>,
    pub native: bool,
    pub exported: bool,
    pub parameters: Vec<Parameter>,
    pub result: bool,
    pub body: Option<Body>,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct Body {
    pub scheduler: bool,
    pub start: bool,
    pub guard: Option<Guard>,
    pub statements: Vec<ParsedStatement>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Guard {
    Modified,
    Scheduled,
}
#[derive(Debug, Clone)]
pub(crate) enum ParsedStatement {
    Let(String, Expression),
    Return(Expression),
    Call(Expression),
}
#[derive(Debug, Clone)]
pub(crate) struct Expression {
    pub span: Range<usize>,
    pub kind: ExpressionKind,
}
#[derive(Debug, Clone)]
pub(crate) enum ExpressionKind {
    Integer(i64),
    Name(String),
    Call(String, Vec<Expression>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueKind {
    Scalar,
    Port,
    Void,
}
#[derive(Debug)]
pub(crate) enum Value {
    Integer(i64),
    Parameter(usize),
    Local(usize),
    Call(usize, Vec<Self>),
}
#[derive(Debug)]
pub(crate) enum Statement {
    Let(Value),
    Return(Value),
    Call(Value),
}
