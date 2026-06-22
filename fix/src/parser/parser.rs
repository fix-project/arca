use super::{Expr, Statement, Token};
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

    pub fn parse_program(&mut self) -> Result<Vec<Statement>, String> {
        let mut program = Vec::new();
        loop {
            self.skip_separators();
            if self.peek(self.position) == Some(&Token::Eof) {
                break;
            }
            program.push(self.parse_statement()?);
            self.skip_separators();
        }
        Ok(program)
    }

    pub fn parse_statement(&mut self) -> Result<Statement, String> {
        // 'print()' is special built-in
        match (self.peek(self.position), self.peek(self.position + 1)) {
            (Some(Token::Identifier(name)), Some(Token::LParen)) if name == "print" => {
                // consume 'print''('
                self.advance();
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen, "expected ')' for print")?;
                Ok(Statement::Print(expr))
            }
            (Some(Token::Identifier(name)), Some(Token::Equals)) => {
                let name = name.clone();
                // consume 'identifier' '='
                self.advance();
                self.advance();
                Ok(Statement::Assign {
                    name,
                    expr: self.parse_expr()?,
                })
            }
            _ => Ok(Statement::Expr(self.parse_expr()?)),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primitive()?;

        while self.matches(&Token::LParen) {
            let Expr::Identifier(name) = expr else {
                return Err(String::from("functions must be named"));
            };

            // arguments for function calls
            let mut args = Vec::new();
            if self.peek(self.position) != Some(&Token::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if !self.matches(&Token::Comma) {
                        break;
                    }
                }
            }
            self.expect(&Token::RParen, "expected ')' for function call")?;
            expr = Expr::Call { name, args };
        }

        Ok(expr)
    }

    fn parse_primitive(&mut self) -> Result<Expr, String> {
        match self.advance() {
            Token::Number(value) => Ok(Expr::Number(value)),
            Token::Identifier(value) => Ok(Expr::Identifier(value)),
            Token::String(value) => Ok(Expr::String(value)),
            Token::LParen => {
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen, "expected ')' for grouping")?;
                Ok(Expr::Group(Box::new(expr)))
            }
            token => Err(format!("unexpected token: {token:?}")),
        }
    }

    fn skip_separators(&mut self) {
        while self.matches(&Token::Semicolon) {}
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
