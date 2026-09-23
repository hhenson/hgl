use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "logos", derive(logos::Logos))]
pub enum Kind {
    #[cfg_attr(feature = "logos", token("module"))]
    Module,
    #[cfg_attr(feature = "logos", token("struct"))]
    Struct,
    #[cfg_attr(feature = "logos", token("fn"))]
    Fn,
    #[cfg_attr(feature = "logos", regex("[a-zA-Z_][a-zA-Z0-9_]*"))]
    Name,
    #[cfg_attr(feature = "logos", regex("[0-9]+"))]
    Int,
    #[cfg_attr(feature = "logos", token("->"))]
    Arrow,
    #[cfg_attr(feature = "logos", token("=>"))]
    FatArrow,
    #[cfg_attr(feature = "logos", token("("))]
    LParen,
    #[cfg_attr(feature = "logos", token(")"))]
    RParen,
    #[cfg_attr(feature = "logos", token("{"))]
    LBrace,
    #[cfg_attr(feature = "logos", token("}"))]
    RBrace,
    #[cfg_attr(feature = "logos", token("<"))]
    Less,
    #[cfg_attr(feature = "logos", token(">"))]
    Greater,
    #[cfg_attr(feature = "logos", token(":"))]
    Colon,
    #[cfg_attr(feature = "logos", token(","))]
    Comma,
    #[cfg_attr(feature = "logos", token("."))]
    Dot,
    #[cfg_attr(feature = "logos", token("+"))]
    Plus,
    #[cfg_attr(feature = "logos", token("*"))]
    Star,
    #[cfg_attr(feature = "logos", regex("[ \t]+"))]
    Space,
    #[cfg_attr(feature = "logos", regex("\r\n|\n|\r"))]
    Newline,
    #[cfg_attr(feature = "logos", token("#", line), token("/*", block))]
    Comment,
    Error,
}
#[cfg(feature = "logos")]
fn line(lex: &mut logos::Lexer<'_, Kind>) {
    let n = lex
        .remainder()
        .find(['\r', '\n'])
        .unwrap_or(lex.remainder().len());
    lex.bump(n);
}
#[cfg(feature = "logos")]
fn block(lex: &mut logos::Lexer<'_, Kind>) -> Result<(), ()> {
    if let Some(n) = lex.remainder().find("*/") {
        lex.bump(n + 2);
        Ok(())
    } else {
        lex.bump(lex.remainder().len());
        Err(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub span: Range<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    I64,
    Ref(Box<Ty>),
    List(Box<Ty>, usize),
    Named(String),
    Bundle(Vec<(String, Ty)>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Name(String),
    Int(i64),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decl {
    Struct(String, Vec<(String, Ty)>),
    Function {
        name: String,
        generics: Vec<String>,
        params: Vec<(String, Ty)>,
        result: Ty,
        body: Expr,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub span: Range<usize>,
    pub message: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub declarations: Vec<Decl>,
    pub issues: Vec<Issue>,
}
