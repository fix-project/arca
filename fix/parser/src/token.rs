use alloc::{string::String, vec::Vec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    Number(i64),
    String(String),
    Bytes(Vec<u8>),
    Primitive(String),
    LParen,
    RParen,
    Ampersand,
    Caret,
    Asterisk,
    Bang,
    Eof,
}
