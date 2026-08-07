use super::lexer::*;
use core::{iter::Peekable, str::Chars};
use kernel::host::fs::{File, Whence};
use kernel::prelude::*;

// Temporary placeholder until the standard library format is finalized
pub const STDLIB: &str = "./fix/stdlib";

pub struct Preprocessor<'a> {
    characters: Peekable<Chars<'a>>,
}

impl<'a> Preprocessor<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            characters: input.chars().peekable(),
        }
    }

    pub fn preprocess(mut self) -> Result<String, String> {
        let mut output = String::new();

        while let Some(character) = self.characters.next() {
            match character {
                // Inline comments
                '-' if self.characters.next_if_eq(&'-').is_some() => {
                    while self.characters.next_if(|&ch| ch != '\n').is_some() {}
                    self.characters.next();
                }
                // Block comments
                '{' if self.characters.next_if_eq(&'-').is_some() => {
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
                            (None, _) => return Err(String::from("unterminated block comment")),
                            _ => {}
                        }
                    }
                }
                '$' => {
                    let mut name = String::new();
                    while let Some(character) =
                        self.characters.next_if(|&ch| Lexer::is_identifier(ch))
                    {
                        name.push(character);
                    }
                    let program = read_file(&format!("{STDLIB}/{name}"))?;
                    output.push_str(&format!("0x{}", hex::encode(program)));
                }
                '@' => {
                    if self.characters.next() != Some('"') {
                        return Err(String::from("expected path after '@'"));
                    }
                    let mut path = String::new();
                    while let Some(next) = self.characters.next_if(|&ch| ch != '"') {
                        path.push(next);
                    }
                    if self.characters.next() != Some('"') {
                        return Err(String::from("unterminated path"));
                    }
                    let program = read_file(&path)?;
                    output.push_str(&format!("0x{}", hex::encode(program)));
                }
                character => output.push(character),
            }
        }
        Ok(output)
    }
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
