#![no_std]
use core::{iter::Peekable, str::Chars};
extern crate alloc;
use alloc::{format, string::String, vec::Vec};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Import {
    Handle(String),
    Label(String),
    File(String),
    Todo {
        reference: Option<String>,
        uris: Vec<String>,
    },
    Binding(String),
    Script {
        location: String,
        positionals: Vec<String>,
        bindings: Vec<(String, String)>,
    },
}

pub struct Preprocessor<'a> {
    characters: Peekable<Chars<'a>>,
}

impl<'a> Preprocessor<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            characters: input.chars().peekable(),
        }
    }

    pub fn preprocess(
        self,
        read_file: impl Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Result<String, String> {
        self.preprocess_with(|import| match import {
            Import::File(path) => read_file(path).map(|bytes| format!("0x{}", hex::encode(bytes))),
            Import::Binding(name) => Ok(format!("${name}")),
            _ => Err(String::from("this import requires a repository")),
        })
    }

    pub fn preprocess_with(
        mut self,
        mut expand_import: impl FnMut(&Import) -> Result<String, String>,
    ) -> Result<String, String> {
        let mut output = String::new();

        while let Some(character) = self.characters.next() {
            match character {
                // Inline comments
                '-' if self.characters.next_if_eq(&'-').is_some() => {
                    while self.characters.next_if(|&ch| ch != '\n').is_some() {}
                }
                // Block comments
                '{' if self.characters.next_if_eq(&'-').is_some() => {
                    output.push(' ');
                    let mut depth = 1;
                    while depth > 0 {
                        match (self.characters.next(), self.characters.peek()) {
                            (Some('{'), Some(&'-')) => {
                                depth += 1;
                                self.characters.next();
                            }
                            (Some('-'), Some(&'}')) => {
                                depth -= 1;
                                self.characters.next();
                            }
                            // Preserve newline for separating statements
                            (Some('\n'), _) => output.push('\n'),
                            (None, _) => return Err(String::from("unterminated block comment")),
                            _ => {}
                        }
                    }
                }
                '"' => {
                    output.push(character);
                    loop {
                        let next = self.characters.next().ok_or("unterminated string")?;
                        output.push(next);
                        if next == '"' {
                            break;
                        }
                        if next == '\\' {
                            output.push(self.characters.next().ok_or("unterminated escape")?);
                        }
                    }
                }
                '@' => {
                    let directive = self.identifier();
                    self.spacing()?;
                    if self.characters.next() != Some('(') {
                        return Err(String::from("expected '(' after import name"));
                    }
                    let first = self.import_string()?;
                    let import = match directive.as_str() {
                        "handle" => Import::Handle(first),
                        "label" => Import::Label(first),
                        "file" => Import::File(first),
                        "todo" => {
                            let mut strings = alloc::vec![first];
                            loop {
                                self.spacing()?;
                                if self.characters.peek() == Some(&')') {
                                    break;
                                }
                                strings.push(self.import_string()?);
                            }
                            let reference = if strings.len() > 1 {
                                Some(strings.remove(0))
                            } else {
                                None
                            };
                            Import::Todo {
                                reference,
                                uris: strings,
                            }
                        }
                        "script" => {
                            let mut positionals = Vec::new();
                            let mut bindings = Vec::new();
                            loop {
                                self.spacing()?;
                                if self.characters.peek() == Some(&')') {
                                    break;
                                }
                                let before = self.characters.clone();
                                let name = self.identifier();
                                self.spacing()?;
                                if !name.is_empty() && self.characters.next_if_eq(&'=').is_some() {
                                    if name.chars().all(|ch| ch.is_ascii_digit()) {
                                        return Err(String::from(
                                            "positional arguments cannot be named",
                                        ));
                                    }
                                    bindings.push((name, self.expression()?));
                                } else {
                                    self.characters = before;
                                    positionals.push(self.expression()?);
                                }
                            }
                            Import::Script {
                                location: first,
                                positionals,
                                bindings,
                            }
                        }
                        _ => return Err(format!("unknown import: @{directive}")),
                    };
                    self.spacing()?;
                    if self.characters.next() != Some(')') {
                        return Err(String::from("expected ')' after import"));
                    }
                    output.push_str(&expand_import(&import)?);
                }
                '$' => {
                    let name = self.identifier();
                    if name.is_empty() {
                        return Err(String::from("expected binding name"));
                    }
                    output.push_str(&expand_import(&Import::Binding(name))?);
                }
                character => output.push(character),
            }
        }
        Ok(output)
    }
    fn identifier(&mut self) -> String {
        let mut name = String::new();
        while let Some(&next) = self.characters.peek() {
            if next == '-' && self.characters.clone().nth(1) == Some('-') {
                break;
            }
            if !(next.is_ascii_alphanumeric() || next == '_' || next == '-') {
                break;
            }
            name.push(self.characters.next().unwrap());
        }
        name
    }

    fn whitespace(&mut self) {
        while self.characters.next_if(|ch| ch.is_whitespace()).is_some() {}
    }

    fn import_string(&mut self) -> Result<String, String> {
        self.spacing()?;
        if self.characters.next() != Some('"') {
            return Err(String::from("expected quoted import"));
        }
        let mut text = String::new();
        loop {
            match self.characters.next().ok_or("unterminated import")? {
                '"' => return Ok(text),
                '\\' => text.push(
                    match self.characters.next().ok_or("unterminated import escape")? {
                        '\\' => '\\',
                        '"' => '"',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        _ => return Err(String::from("invalid import escape")),
                    },
                ),
                ch => text.push(ch),
            }
        }
    }
    fn spacing(&mut self) -> Result<(), String> {
        loop {
            self.whitespace();
            match (
                self.characters.peek().copied(),
                self.characters.clone().nth(1),
            ) {
                (Some('-'), Some('-')) => {
                    while self.characters.next_if(|ch| *ch != '\n').is_some() {}
                }
                (Some('{'), Some('-')) => {
                    self.characters.next();
                    self.characters.next();
                    let mut depth = 1;
                    while depth != 0 {
                        match (self.characters.next(), self.characters.peek().copied()) {
                            (Some('{'), Some('-')) => {
                                self.characters.next();
                                depth += 1;
                            }
                            (Some('-'), Some('}')) => {
                                self.characters.next();
                                depth -= 1;
                            }
                            (None, _) => return Err(String::from("unterminated block comment")),
                            _ => {}
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn expression(&mut self) -> Result<String, String> {
        self.spacing()?;
        let first = self
            .characters
            .next()
            .ok_or("expected binding expression")?;
        let mut text = String::from(first);
        match first {
            '"' => loop {
                let next = self.characters.next().ok_or("unterminated string")?;
                text.push(next);
                if next == '"' {
                    break;
                }
                if next == '\\' {
                    text.push(self.characters.next().ok_or("unterminated escape")?);
                }
            },
            '(' | '[' | '{' => {
                let close = match first {
                    '(' => ')',
                    '[' => ']',
                    _ => '}',
                };
                loop {
                    self.spacing()?;
                    if self.characters.next_if_eq(&close).is_some() {
                        text.push(close);
                        break;
                    }
                    text.push_str(&self.expression()?);
                    text.push(' ');
                }
            }
            '&' | '\'' | '#' | '*' | '+' => text.push_str(&self.expression()?),
            '@' => {
                text.push_str(&self.identifier());
                self.spacing()?;
                if self.characters.peek() != Some(&'(') {
                    return Err(String::from("expected import arguments"));
                }
                text.push_str(&self.expression()?);
            }
            ')' | ']' | '}' => return Err(String::from("expected binding expression")),
            _ => {
                while let Some(&next) = self.characters.peek() {
                    if next.is_whitespace()
                        || "()[]{}".contains(next)
                        || (next == '-' && self.characters.clone().nth(1) == Some('-'))
                    {
                        break;
                    }
                    text.push(self.characters.next().unwrap());
                }
            }
        }
        Ok(text)
    }
}
