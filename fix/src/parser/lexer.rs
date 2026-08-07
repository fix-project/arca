use super::token::Token;
use core::{iter::Peekable, str::Chars};
use kernel::prelude::*;

pub struct Lexer<'a> {
    characters: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            characters: input.chars().peekable(),
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            if token == Token::Eof {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }
        Ok(tokens)
    }

    pub fn next_token(&mut self) -> Result<Token, String> {
        // skip whitespace
        self.take(String::new(), |ch| ch.is_whitespace());
        let Some(character) = self.characters.next() else {
            return Ok(Token::Eof);
        };

        let token = match character {
            '(' => Token::LParen,
            ')' => Token::RParen,
            '&' => Token::Ampersand,
            '*' => Token::Asterisk,
            '^' => Token::Caret,
            '!' => Token::Bang,
            '"' => {
                let text = self.take(String::new(), |ch| ch != '"');
                if self.characters.next() != Some('"') {
                    return Err(String::from("unterminated string"));
                }
                Token::String(text)
            }
            '0' if self.peek(|character| *character == 'x') => {
                self.characters.next();
                let digits = self.take(String::new(), |ch| ch.is_ascii_hexdigit());
                Token::Bytes(hex::decode(&digits).map_err(|error| error.to_string())?)
            }
            // Negative numbers
            '-' if self.peek(|character| character.is_ascii_digit()) => {
                let number = self.take(String::new(), |ch| ch.is_ascii_digit());
                Token::Number(-number.parse::<i64>().map_err(|error| error.to_string())?)
            }
            character if character.is_ascii_digit() => {
                let number = self.take(String::from(character), |ch| ch.is_ascii_digit());
                Token::Number(number.parse::<i64>().map_err(|error| error.to_string())?)
            }
            character if Self::is_identifier(character) => {
                Token::Identifier(self.take(String::from(character), Self::is_identifier))
            }
            other => return Err(format!("unexpected character: {other:?}")),
        };
        Ok(token)
    }

    fn peek<F>(&mut self, function: F) -> bool
    where
        F: FnOnce(&char) -> bool,
    {
        self.characters.peek().is_some_and(function)
    }

    fn take<F>(&mut self, mut text: String, mut condition: F) -> String
    where
        F: FnMut(char) -> bool,
    {
        while let Some(next) = self.characters.next_if(|&ch| condition(ch)) {
            text.push(next);
        }
        text
    }

    pub fn is_identifier(character: char) -> bool {
        character.is_ascii_alphabetic() || character == '_'
    }
}
