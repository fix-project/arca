use core::clone::Clone;
use kernel::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    Number(i64),
    String(String),
    Bytes(Vec<u8>),
    LParen,
    RParen,
    Ampersand,
    Caret,
    Asterisk,
    Bang,
    Eof,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(i64),
    String(String),
    Bytes(Vec<u8>),
    Identifier(String),
    Ref(Box<Expr>),
    Tree(Vec<Expr>),
    Application(Box<Expr>),
    Identification(Box<Expr>),
    StrictEncode(Box<Expr>),
    Let {
        bindings: Vec<(String, Expr)>,
        body: Box<Expr>,
    },
}
