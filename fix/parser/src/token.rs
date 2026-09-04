use alloc::{string::String, vec::Vec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    String(String),
    Bytes(Vec<u8>),
    Primitive(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Ampersand,
    Apostrophe,
    Asterisk,
    Plus,
    Pound,
    Eof,
}
