use super::token::Token;
use core::iter::Peekable;
use core::str::Chars;
use kernel::host::fs::{File, Whence};
use kernel::prelude::*;

pub struct Lexer<'a> {
    characters: Peekable<Chars<'a>>,
}

// Temporary placeholder until the standard library format is finalized
const STDLIB: &str = "./fix/stdlib/";

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            characters: input.chars().peekable(),
        }
    }

    pub fn preprocess(file: &str) -> Result<String, String> {
        let mut output = String::new();
        let mut characters = file.chars().peekable();

        while let Some(character) = characters.next() {
            match character {
                '$' => {
                    let mut name = String::new();
                    while let Some(character) = characters.next_if(|&ch| Self::is_identifier(ch)) {
                        name.push(character);
                    }
                    let program = Self::read_file(&format!("{STDLIB}/{name}"))?;
                    output.push_str(&format!("0x{}", hex::encode(program)));
                }
                '@' => {
                    if characters.next() != Some('"') {
                        return Err(String::from("expected path after '@'"));
                    }
                    let mut path = String::new();
                    while let Some(next) = characters.next_if(|&ch| ch != '"') {
                        path.push(next);
                    }
                    if characters.next() != Some('"') {
                        return Err(String::from("unterminated path"));
                    }
                    let program = Self::read_file(&path)?;
                    output.push_str(&format!("0x{}", hex::encode(program)));
                }
                character => output.push(character),
            }
        }
        Ok(output)
    }

    pub fn read_file(path: &str) -> Result<Vec<u8>, String> {
        let mut file = File::open(path, true, false, false, false, false)
            .map_err(|_| format!("could not open {path}"))?;
        let len = file.seek(Whence::End(0)) as usize;
        file.seek(Whence::Start(0));
        let mut data = vec![0; len];
        file.read_exact(&mut data);
        Ok(data)
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
            // Inline comments
            '/' => {
                if self.characters.next() == Some('/') {
                    self.take(String::new(), |ch| ch != '\n');
                    return self.next_token();
                }
                return Err(String::from("unexpected character: '/'"));
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

    fn is_identifier(character: char) -> bool {
        character.is_ascii_alphabetic() || character == '_'
    }
}
