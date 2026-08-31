use alloc::{collections::BTreeMap, string::String, vec::Vec};

use crate::token::Token;
use fixutils::*;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    environment: BTreeMap<String, RustHandle<'static>>,
    context: BTreeMap<String, RustHandle<'static>>,
}

impl Parser {
    pub fn new(
        tokens: Vec<Token>,
        environment_handle: &RustHandle<'static>,
    ) -> Result<Self, Error> {
        let mut environment = BTreeMap::new();
        for entry in environment_handle.to_entries()? {
            let entry = entry.to_entries()?;
            let name = *entry.first().expect("expect name");
            let object = entry.get(1).expect("expect object");
            environment.insert(
                String::from_utf8(name.to_bytes()?).expect("valid name"),
                *object,
            );
        }

        Ok(Self {
            tokens,
            position: 0,
            environment,
            context: BTreeMap::new(),
        })
    }

    pub fn parse_program(&mut self) -> Result<RustHandle<'static>, Error> {
        let handle = self.parse_expr()?;
        self.expect(&Token::Eof, "expected end of program");
        Ok(handle)
    }

    fn parse_expr(&mut self) -> Result<RustHandle<'static>, Error> {
        Ok(match self.advance() {
            Token::String(string) => RustHandle::from_bytes(string.as_bytes())?,
            Token::Bytes(bytes) => RustHandle::from_bytes(&bytes)?,
            Token::Identifier(name) => *self.context.get(&name).expect("undefined identifier"),
            Token::Primitive(name) => *self.environment.get(&name).expect("undefined primitive"),
            Token::Ampersand => create_ref(self.parse_expr()?),
            Token::Apostrophe => create_identification_thunk(self.parse_expr()?),
            Token::Pound => create_application_thunk(self.parse_expr()?),
            Token::Asterisk => create_strict_encode(self.parse_expr()?),
            Token::Plus => create_shallow_encode(self.parse_expr()?),
            Token::LParen => {
                if let Some(Token::Identifier(token)) = self.peek(self.position)
                    && token == "let"
                {
                    self.advance();
                    self.parse_let()?
                } else {
                    RustHandle::from_entries(&self.parse_handles(&Token::RParen)?)?
                }
            }
            Token::LBracket => create_selection_thunk(RustHandle::from_entries(
                &self.parse_handles(&Token::RBracket)?,
            )?),
            token => panic!("unexpected token: {token:?}"),
        })
    }

    fn parse_handles(&mut self, close: &Token) -> Result<Vec<RustHandle<'static>>, Error> {
        let mut handles = Vec::new();
        while !self.matches(close) {
            handles.push(self.parse_expr()?);
        }
        Ok(handles)
    }

    fn parse_let(&mut self) -> Result<RustHandle<'static>, Error> {
        self.expect(&Token::LParen, "expected '(' for let bindings");
        let outer_context = self.context.clone();
        while self.matches(&Token::LParen) {
            let Token::Identifier(name) = self.advance() else {
                panic!("expected name in let binding")
            };
            let handle = self.parse_expr()?;
            self.expect(&Token::RParen, "expected ')' for let binding");
            self.context.insert(name, handle);
        }
        self.expect(&Token::RParen, "expected ')' for let bindings");
        let body = self.parse_expr()?;
        self.expect(&Token::RParen, "expected ')' for let");
        self.context = outer_context;
        Ok(body)
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
