use kernel::prelude::*;
use core::{clone::Clone, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    Number(i64),
    String(String),
    LParen,
    RParen,
    Comma,
    Semicolon,
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
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assign { name: String, expr: Expr },
    Print(Expr),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Value<H, B, T> {
    Handle(H),
    BlobData(B),
    TreeData(T),
    Int(i64),
    String(String),
    Unit,
}

impl<H, B, T> fmt::Display for Value<H, B, T>
where
    H: fmt::Debug,
    B: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handle(handle) => write!(formatter, "{handle:?}"),
            Self::BlobData(data) => write!(formatter, "{data:?}"),
            Self::TreeData(data) => write!(formatter, "{data:?}"),
            Self::Int(value) => write!(formatter, "{value}"),
            Self::String(value) => write!(formatter, "{value}"),
            Self::Unit => write!(formatter, "()"),
        }
    }
}
