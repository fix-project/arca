use alloc::{collections::BTreeMap, string::String, vec::Vec};

use crate::token::Token;
use fixwasm::{Any, Blob, Error, Handle, Object, Thunk, Tree, Value};

type Parsed = Handle<Any>;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    environment: BTreeMap<String, Parsed>,
    context: BTreeMap<String, Parsed>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, combination: &Handle<Object<Tree>>) -> Result<Self, Error> {
        let primitives = Handle::<Object<Tree>>::try_from(combination.get(3))?;
        let mut environment: BTreeMap<String, Parsed> = BTreeMap::new();
        for index in 0..usize::try_from(primitives.len()?)? {
            let pair = Handle::<Object<Tree>>::try_from(primitives.get(index))?;
            assert!(pair.len()? >= 2, "expected environment pair");
            let name = String::from_utf8(Handle::<Object<Blob>>::try_from(pair.get(0))?.read()?)
                .expect("valid name");
            environment.insert(name, pair.get(1));
        }

        Ok(Self {
            tokens,
            position: 0,
            environment,
            context: BTreeMap::new(),
        })
    }

    pub fn parse_program(mut self) -> Result<Parsed, Error> {
        self.parse_body(&Token::Eof)
    }

    fn parse_body(&mut self, close: &Token) -> Result<Parsed, Error> {
        let expression = loop {
            while self.matches(&Token::Newline) {}
            if let Some(Token::Identifier(name)) = self.peek(self.position).cloned()
                && self.peek(self.position + 1) == Some(&Token::Equal)
            {
                self.position += 2;
                let expression = self.parse_expr()?;
                self.expect(&Token::Newline, "expected newline after assignment");
                self.context.insert(name, expression);
                continue;
            }
            let expression = self.parse_expr()?;
            while self.matches(&Token::Newline) {}
            self.expect(close, "expected end of script");
            break expression;
        };
        Ok(expression)
    }

    fn parse_expr(&mut self) -> Result<Parsed, Error> {
        Ok(match self.advance() {
            Token::String(string) => Blob::create(string.into_bytes()).erase(),
            Token::Bytes(bytes) => Blob::create(bytes).erase(),
            Token::Identifier(name) => self
                .context
                .get(&name)
                .expect("undefined identifier")
                .clone(),
            Token::Primitive(name) => self
                .environment
                .get(&name)
                .expect("undefined primitive")
                .clone(),
            Token::Ampersand => {
                let value: Handle<Object<Any>> = self.parse_expr()?.try_into()?;
                value.reference().erase()
            }
            Token::Apostrophe => self.parse_expr()?.identification().erase(),
            Token::Pound => self.parse_expr()?.application().erase(),
            Token::Asterisk => {
                let value: Handle<Thunk> = self.parse_expr()?.try_into()?;
                value.strict().erase()
            }
            Token::Plus => {
                let value: Handle<Thunk> = self.parse_expr()?.try_into()?;
                value.shallow().erase()
            }
            Token::LBrace => {
                let environment = self.environment.clone();
                let mut bindings = Vec::new();
                while self.matches(&Token::Newline) {}
                while let Some(Token::Primitive(name)) = self.peek(self.position).cloned()
                    && self.peek(self.position + 1) == Some(&Token::Equal)
                {
                    self.position += 2;
                    bindings.push((name, self.parse_expr()?));
                    self.expect(&Token::Newline, "expected newline after binding");
                    while self.matches(&Token::Newline) {}
                }
                let context = core::mem::take(&mut self.context);
                self.environment.extend(bindings);
                let expression = self.parse_body(&Token::RBrace);
                self.context = context;
                self.environment = environment;
                expression?
            }
            Token::LParen => Tree::from_iter(self.parse_handles(&Token::RParen)?).erase(),
            Token::LBracket => Tree::from_iter(self.parse_handles(&Token::RBracket)?)
                .reference()
                .selection()
                .erase(),
            token => panic!("unexpected token: {token:?}"),
        })
    }

    fn parse_handles(&mut self, close: &Token) -> Result<Vec<Parsed>, Error> {
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
        let token = self
            .tokens
            .get_mut(self.position)
            .map(|token| core::mem::replace(token, Token::Eof))
            .unwrap_or(Token::Eof);
        self.position += 1;
        token
    }
}
