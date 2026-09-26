use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};

use crate::token::Token;
use fixutils::*;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    environment: BTreeMap<String, HandleOp<'static>>,
    context: BTreeMap<String, HandleOp<'static>>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, environment_handle: TableGet<'static>) -> Result<Self, Error> {
        let mut environment = BTreeMap::new();
        for entry in environment_handle.to_entries()? {
            let entry = entry.to_entries()?;
            let name = *entry.first().expect("expect name");
            let object = *entry.get(1).expect("expect object");
            environment.insert(
                String::from_utf8(name.to_bytes()?).expect("valid name"),
                object.into(),
            );
        }

        Ok(Self {
            tokens,
            position: 0,
            environment,
            context: BTreeMap::new(),
        })
    }

    pub fn parse_program(&mut self) -> Result<HandleOp<'static>, Error> {
        loop {
            // skip empty lines
            while self.matches(&Token::Newline) {}

            if let Some(Token::Identifier(name)) = self.peek(self.position).cloned()
                && self.peek(self.position + 1) == Some(&Token::Equal)
            {
                self.position += 2;
                let handle = self.parse_expr()?;
                self.expect(&Token::Newline, "expected newline after assignment");
                self.context.insert(name, handle);
                continue;
            }

            let handle = self.parse_expr()?;
            // skip empty lines
            while self.matches(&Token::Newline) {}
            self.expect(&Token::Eof, "expected end of program");
            return Ok(handle);
        }
    }

    fn parse_expr(&mut self) -> Result<HandleOp<'static>, Error> {
        Ok(match self.advance() {
            Token::String(string) => from_bytes(string.as_bytes())?.into(),
            Token::Bytes(bytes) => from_bytes(&bytes)?.into(),
            Token::Identifier(name) => *self.context.get(&name).expect("undefined identifier"),
            Token::Primitive(name) => *self.environment.get(&name).expect("undefined primitive"),
            Token::Ampersand => HandleOp::CreateRef(self.previous()?),
            Token::Apostrophe => HandleOp::Identification(self.previous()?),
            Token::Pound => HandleOp::Application(self.previous()?),
            Token::Asterisk => HandleOp::StrictEncode(self.previous()?),
            Token::Plus => HandleOp::ShallowEncode(self.previous()?),
            Token::LParen => from_entries(&self.parse_handles(&Token::RParen)?)?.into(),
            Token::LBracket => HandleOp::Selection(Box::leak(Box::new(
                from_entries(&self.parse_handles(&Token::RBracket)?)?.into(),
            ))),
            token => panic!("unexpected token: {token:?}"),
        })
    }

    fn previous(&mut self) -> Result<&'static HandleOp<'static>, Error> {
        Ok(Box::leak(Box::new(self.parse_expr()?)))
    }

    fn parse_handles(&mut self, close: &Token) -> Result<Vec<HandleOp<'static>>, Error> {
        let mut handles = Vec::new();
        while !self.matches(close) {
            handles.push(self.parse_expr()?);
        }
        Ok(handles)
    }

    fn expect(&mut self, token: &Token, message: &str) {
        assert!(self.matches(token), "{message}");
    }

    fn matches(&mut self, token: &Token) -> bool {
        if self.peek(self.position) == Some(token) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self, position: usize) -> Option<&Token> {
        self.tokens.get(position)
    }

    fn advance(&mut self) -> Token {
        let token = self.peek(self.position).cloned().unwrap_or(Token::Eof);
        self.position += 1;
        token
    }
}
