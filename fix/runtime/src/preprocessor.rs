use core::{iter::Peekable, str::Chars};
extern crate alloc;
use alloc::{format, string::String, vec::Vec};

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
        self.preprocess_with(|path| {
            read_file(path).map(|bytes| format!("0x{}", hex::encode(bytes)))
        })
    }

    pub fn preprocess_with(
        mut self,
        mut expand_file: impl FnMut(&str) -> Result<String, String>,
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
                    if self.characters.next() != Some('"') {
                        return Err(String::from("expected path after '@'"));
                    }
                    let mut path = String::new();
                    while let Some(next) = self.characters.next_if(|&ch| ch != '"') {
                        path.push(if next == '\\' {
                            match self.characters.next().ok_or("unterminated path escape")? {
                                '\\' => '\\',
                                '"' => '"',
                                'n' => '\n',
                                'r' => '\r',
                                't' => '\t',
                                _ => return Err(String::from("invalid path escape")),
                            }
                        } else {
                            next
                        });
                    }
                    if self.characters.next() != Some('"') {
                        return Err(String::from("unterminated path"));
                    }
                    output.push_str(&expand_file(&path)?);
                }
                character => output.push(character),
            }
        }
        Ok(output)
    }
}
