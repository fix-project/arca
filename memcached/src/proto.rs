use alloc::{string::String, vec::Vec};
use chumsky::prelude::*;

#[derive(Debug)]
pub enum Command {
    Set,
    Add,
    Replace,
    Append,
    Prepend,
}

#[derive(Debug)]
pub struct Storage {
    pub command: Command,
    pub key: String,
    pub flags: u16,
    pub exptime: u32,
    pub value: usize,
}

fn command<'a>() -> impl Parser<'a, &'a [u8], Command, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    choice((just(b"set").map(|_| Command::Set),)).labelled("command")
}

fn ident<'a>() -> impl Parser<'a, &'a [u8], String, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    any()
        .filter(|x: &u8| x.is_ascii_alphanumeric() || *x == b'-')
        .repeated()
        .at_least(1)
        .collect()
        .map(|x: Vec<u8>| String::from_utf8(x))
        .unwrapped()
        .labelled("ident")
}

fn parse_u16<'a>() -> impl Parser<'a, &'a [u8], u16, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    any()
        .filter(|x: &u8| x.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect()
        .map(|x: Vec<u8>| String::from_utf8(x))
        .unwrapped()
        .map(|x: String| x.parse())
        .unwrapped()
        .labelled("u16")
}

fn parse_u32<'a>() -> impl Parser<'a, &'a [u8], u32, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    any()
        .filter(|x: &u8| x.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect()
        .map(|x: Vec<u8>| String::from_utf8(x))
        .unwrapped()
        .map(|x: String| x.parse())
        .unwrapped()
        .labelled("u32")
}

fn parse_usize<'a>() -> impl Parser<'a, &'a [u8], usize, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    any()
        .filter(|x: &u8| x.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect()
        .map(|x: Vec<u8>| String::from_utf8(x))
        .unwrapped()
        .map(|x: String| x.parse())
        .unwrapped()
        .labelled("usize")
}

fn storage<'a>() -> impl Parser<'a, &'a [u8], Storage, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    command()
        .then_ignore(just(b' '))
        .then(ident().then_ignore(just(b' ')))
        .then(parse_u16().then_ignore(just(b' ')))
        .then(parse_u32().then_ignore(just(b' ')))
        .then(parse_usize().then_ignore(just(b"\r\n")))
        .map(|((((command, key), flags), exptime), value)| Storage {
            command,
            key,
            flags,
            exptime,
            value,
        })
        .labelled("storage request")
}

#[derive(Debug)]
pub enum Request {
    Storage(Storage),
    Get(Vec<String>),
    Delete(String),
}

fn get<'a>() -> impl Parser<'a, &'a [u8], Vec<String>, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    just(b"get ")
        .ignore_then(
            ident()
                .then_ignore(just(b' ').or_not())
                .repeated()
                .collect(),
        )
        .then_ignore(just(b"\r\n"))
        .labelled("get request")
}

fn delete<'a>() -> impl Parser<'a, &'a [u8], String, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    just(b"delete ")
        .ignore_then(ident())
        .then_ignore(just(b"\r\n"))
        .labelled("delete request")
}

pub fn request<'a>() -> impl Parser<'a, &'a [u8], Request, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    choice((
        storage().map(Request::Storage),
        get().map(Request::Get),
        delete().map(Request::Delete),
    ))
    .labelled("request")
}
