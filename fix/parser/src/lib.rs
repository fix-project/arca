#![cfg_attr(target_arch = "wasm32", no_std, feature(asm_experimental_arch))]
extern crate alloc;

use dlmalloc::GlobalDlmalloc;
#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;

mod lexer;
mod parser;
mod token;

use fixutils::*;
use lexer::Lexer;
use parser::Parser;

#[procedure_entrypoint]
pub fn _fixpoint_apply(combination: Combination) -> Result<HandleOp<'static>, Error> {
    let arguments = combination.to_entries()?;

    let source = arguments.get(1).expect("expected source").to_bytes()?;
    let source = core::str::from_utf8(&source).expect("source should be valid UTF-8");

    let tokens = Lexer::new(source).tokenize().expect("failed to tokenize");
    let mut parser = Parser::new(tokens, *arguments.get(2).expect("expected environment"))?;
    parser.parse_program()
}
