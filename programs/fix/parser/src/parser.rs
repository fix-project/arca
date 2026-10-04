use alloc::{collections::BTreeMap, rc::Rc, string::String, vec::Vec};

use crate::token::Token;
use fix::{Any, Blob, Cast, Error, Handle, Object, Thunk, Tree, Value};

type Parsed<'a> = Rc<dyn Value<Type = Any> + 'a>;

pub struct Parser<'a> {
    tokens: Vec<Token>,
    position: usize,
    environment: BTreeMap<String, Parsed<'a>>,
    context: BTreeMap<String, Parsed<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(
        tokens: Vec<Token>,
        combination: &'a Handle<Object<Tree>, fix::Focus>,
    ) -> Result<Self, Error> {
        let primitives = Handle::<Object<Tree>, _>::try_from(combination.get(2))?;
        let mut environment: BTreeMap<String, Parsed<'a>> = BTreeMap::new();
        for index in 0..primitives.len()? {
            let pair = Handle::<Object<Tree>, _>::try_from(primitives.get(index))?;
            assert!(pair.len()? >= 2, "expected environment pair");
            let name = String::from_utf8(Handle::<Object<Blob>, _>::try_from(pair.get(0))?.read()?)
                .expect("valid name");
            environment.insert(name, Rc::new(pair.get(1)));
        }

        Ok(Self {
            tokens,
            position: 0,
            environment,
            context: BTreeMap::new(),
        })
    }

    pub fn parse_program(mut self) -> Result<Parsed<'a>, Error> {
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
            self.expect(&Token::Eof, "expected end of program");
            break expression;
        };
        Ok(expression)
    }

    fn parse_expr(&mut self) -> Result<Parsed<'a>, Error> {
        Ok(match self.advance() {
            Token::String(string) => Rc::new(Blob::create(string.into_bytes()).erase()),
            Token::Bytes(bytes) => Rc::new(Blob::create(bytes).erase()),
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
                let value: Cast<Object<Any>, _> = self.parse_expr()?.try_into()?;
                Rc::new(value.reference().erase())
            }
            Token::Apostrophe => Rc::new(self.parse_expr()?.identification().erase()),
            Token::Pound => Rc::new(self.parse_expr()?.application().erase()),
            Token::Asterisk => {
                let value: Cast<Thunk, _> = self.parse_expr()?.try_into()?;
                Rc::new(value.strict().erase())
            }
            Token::Plus => {
                let value: Cast<Thunk, _> = self.parse_expr()?.try_into()?;
                Rc::new(value.shallow().erase())
            }
            Token::LParen => Rc::new(Tree::create(self.parse_handles(&Token::RParen)?).erase()),
            Token::LBracket => Rc::new(
                Tree::create(self.parse_handles(&Token::RBracket)?)
                    .reference()
                    .selection()
                    .erase(),
            ),
            token => panic!("unexpected token: {token:?}"),
        })
    }

    fn parse_handles(&mut self, close: &Token) -> Result<Vec<Parsed<'a>>, Error> {
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
