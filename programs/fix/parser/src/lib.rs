#![cfg_attr(target_arch = "wasm32", no_std)]
extern crate alloc;

use dlmalloc::GlobalDlmalloc;
#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;

mod lexer;
mod parser;
mod token;

use fix::{Any, Blob, Error, Handle, Object, Tree, Value};
use lexer::Lexer;
use parser::Parser;

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Any> + 'a, Error> {
    let source = Handle::<Object<Blob>, _>::try_from(combination.get(1))?.read()?;
    let source = core::str::from_utf8(&source).expect("source should be valid UTF-8");
    let tokens = Lexer::new(source).tokenize().expect("failed to tokenize");
    Parser::new(tokens, combination)?.parse_program()
}
