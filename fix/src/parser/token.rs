use core::clone::Clone;
use kernel::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    Number(i64),
    String(String),
    LParen,
    RParen,
    Comma,
    Semicolon,
    Ampersand,
    Asterisk,
    Equals,
    Eof,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(i64),
    Identifier(String),
    String(String),
    Call { name: String, args: Vec<Expr> },
    Group(Box<Expr>),
    Ref(Box<Expr>),
    IdentificationThunk(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assign { name: String, expr: Expr },
    Print(Expr),
    Expr(Expr),
}
