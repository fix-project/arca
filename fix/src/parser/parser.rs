use super::{Expr, Token};
use kernel::prelude::*;

pub struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    pub fn parse_program(&mut self) -> Result<Expr, String> {
        let expr = self.parse_expr()?;
        self.expect(&Token::Eof, "expected end of program")?;
        Ok(expr)
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        match self.advance() {
            Token::Number(number) => Ok(Expr::Number(number)),
            Token::String(string) => Ok(Expr::String(string)),
            Token::Bytes(bytes) => Ok(Expr::Bytes(bytes)),
            Token::Identifier(value) => Ok(Expr::Identifier(value)),
            Token::Ampersand => Ok(Expr::Ref(Box::new(self.parse_expr()?))),
            Token::Caret => Ok(Expr::Identification(Box::new(self.parse_expr()?))),
            Token::Asterisk => Ok(Expr::Application(Box::new(self.parse_expr()?))),
            Token::Bang => Ok(Expr::StrictEncode(Box::new(self.parse_expr()?))),
            Token::LParen => {
                if let Some(Token::Identifier(token)) = self.peek(self.position)
                    && token == "let"
                {
                    self.advance();
                    self.parse_let()
                } else {
                    Ok(Expr::Tree(self.parse_handles()?))
                }
            }
            token => Err(format!("unexpected token: {token:?}")),
        }
    }

    fn parse_handles(&mut self) -> Result<Vec<Expr>, String> {
        let mut handles = Vec::new();
        while !self.matches(&Token::RParen) {
            handles.push(self.parse_expr()?);
        }
        Ok(handles)
    }

    fn parse_let(&mut self) -> Result<Expr, String> {
        self.expect(&Token::LParen, "expected '(' for let bindings")?;
        let mut bindings = Vec::new();
        while self.matches(&Token::LParen) {
            let Token::Identifier(name) = self.advance() else {
                return Err(String::from("expected name in let binding"));
            };
            let value = self.parse_expr()?;
            self.expect(&Token::RParen, "expected ')' for let binding")?;
            bindings.push((name, value));
        }
        self.expect(&Token::RParen, "expected ')' for let bindings")?;
        let body = Box::new(self.parse_expr()?);
        self.expect(&Token::RParen, "expected ')' for let")?;
        Ok(Expr::Let { bindings, body })
    }

    fn expect(&mut self, token: &Token, message: &str) -> Result<(), String> {
        if self.matches(token) {
            Ok(())
        } else {
            Err(String::from(message))
        }
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
