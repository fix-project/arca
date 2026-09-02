use crate::token::Token;
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::{iter::Peekable, str::Chars};

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
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            '&' => Token::Ampersand,
            '*' => Token::Asterisk,
            '+' => Token::Plus,
            '\'' => Token::Apostrophe,
            '#' => Token::Pound,
            '$' => Token::Primitive(self.take(String::new(), Self::is_identifier)),
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
            character if character.is_ascii_digit() => {
                let digits = self.take(String::from(character), |ch| {
                    ch.is_ascii_digit() || ch == '_'
                });
                let suffix = self.take(String::new(), |ch| ch.is_ascii_alphanumeric());
                let bytes = match suffix.as_str() {
                    "u8" => digits.parse::<u8>().map(|n| n.to_le_bytes().to_vec()),
                    "u16" => digits.parse::<u16>().map(|n| n.to_le_bytes().to_vec()),
                    "u32" => digits.parse::<u32>().map(|n| n.to_le_bytes().to_vec()),
                    "u64" => digits.parse::<u64>().map(|n| n.to_le_bytes().to_vec()),
                    "u128" => digits.parse::<u128>().map(|n| n.to_le_bytes().to_vec()),
                    "" => return Err(format!("integer literal {digits} missing suffix")),
                    other => return Err(format!("unknown integer suffix: {other}")),
                };
                Token::Bytes(bytes.map_err(|error| error.to_string())?)
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
