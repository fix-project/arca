#![no_std]
extern crate alloc;

use alloc::{format, rc::Rc, vec, vec::Vec};
use fix::{Any, Blob, Cast, Error, Handle, Object, Thunk, Tree, Value, tree};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

type Expression<'a> = Rc<dyn Value<Type = Any> + 'a>;

fn value<'a>(value: impl Value + 'a) -> Expression<'a> {
    Rc::new(value.erase())
}
fn text<'a>(text: &str) -> Expression<'a> {
    value(Blob::create(text.as_bytes().to_vec()))
}
fn children<'a>(values: Vec<Expression<'a>>) -> Expression<'a> {
    value(Tree::create(values))
}
fn child(value: Expression<'_>, index: usize) -> Result<Expression<'_>, Error> {
    value.with(index, |child| Ok(Rc::new(child) as Expression<'_>))
}
fn application(arguments: Vec<Expression<'_>>) -> Expression<'_> {
    value(Tree::create(arguments).application())
}
fn strict(thunk: Expression<'_>) -> Result<Expression<'_>, Error> {
    let thunk: Cast<Thunk, _> = thunk.try_into()?;
    Ok(value(thunk.strict()))
}
fn stat(filetype: u8) -> Expression<'static> {
    value(tree![
        0u64,
        Blob::create([filetype]),
        1u64,
        0u64,
        0u64,
        0u64,
        0u64
    ])
}
fn descriptor<'a>(name: &str, filetype: u8, contents: Expression<'a>) -> Expression<'a> {
    children(vec![text(name), stat(filetype), contents])
}
fn command<'a>(
    executable: Expression<'a>,
    arguments: Vec<Expression<'a>>,
    toolchain: Expression<'a>,
    directory: Expression<'a>,
) -> Result<Expression<'a>, Error> {
    let stream = |name| value(tree![Blob::create(name), tree![], Blob::create(b"")]);
    strict(application(vec![
        text(""),
        executable,
        children(arguments),
        children(vec![
            text("TMPDIR=/work"),
            text("LC_ALL=C"),
            text("SOURCE_DATE_EPOCH=0"),
        ]),
        children(vec![
            stream(b"stdin" as &[u8]),
            stream(b"stdout"),
            stream(b"stderr"),
            toolchain,
            directory,
        ]),
        value(0u64),
        value(0u64),
    ]))
}
fn work(result: Expression<'_>) -> Result<Expression<'_>, Error> {
    child(child(result, 0)?, 4)
}
fn output<'a>(result: Expression<'a>, name: &str) -> Result<Expression<'a>, Error> {
    let files = child(work(result)?, 2)?;
    for index in 0..usize::try_from(files.len()?)? {
        let file = child(files.clone(), index)?;
        if child(file.clone(), 0)?.equals(&text(name))? {
            return child(file, 2);
        }
    }
    Err(Error::OutOfBounds)
}
fn flags() -> Vec<Expression<'static>> {
    [
        "/toolchain/usr/bin/clang",
        "--target=x86_64-unknown-linux-gnu",
        "-O2",
        "-fno-optimize-sibling-calls",
        "-frounding-math",
        "-ffreestanding",
        "-nostdlib",
        "-nostdinc",
        "-isystem",
        "/toolchain/usr/lib/clang/23/include",
        "-isystem",
        "/toolchain/usr/include/wasm32-wasip1",
        "-isystem",
        "/toolchain/usr/include",
        "-mcmodel=large",
        "-mno-red-zone",
        "-march=x86-64-v3",
        "-static",
        "-resource-dir=/toolchain/usr/lib/clang/23",
        "-no-canonical-prefixes",
        "-I/toolchain/share/fix",
        "-I/work",
    ]
    .into_iter()
    .map(text)
    .collect()
}
fn authenticated<'a>(
    tag: Expression<'a>,
    author: &Expression<'_>,
    claim: &str,
) -> Result<Expression<'a>, Error> {
    assert!(tag.is_tag()? && tag.len()? == 3);
    assert!(child(tag.clone(), 2)?.equals(&text(claim))?);
    let authors = child(tag.clone(), 0)?;
    assert!(authors.len()? == 1 && child(authors, 0)?.equals(author)?);
    child(tag, 1)
}

#[fix::apply]
fn apply<'a>(combination: &'a Handle<Object<Tree>, fix::Focus>) -> Result<Expression<'a>, Error> {
    let closure = value(combination.get(1));
    let identity = child(child(closure.clone(), 1)?, 1)?;
    let resources = authenticated(child(closure.clone(), 2)?, &identity, "Bootstrap")?;
    let wasm2c = child(resources.clone(), 0)?;
    let clang = child(resources.clone(), 1)?;
    let toolchain = child(resources, 2)?;
    let input = combination.get(2);
    let (wasm, partitions, phase, results) = if input.value_type()? == fix::ValueType::Object
        && input.data_type()? == fix::DataType::Blob
    {
        assert_eq!(combination.len()?, 4);
        (
            value(input),
            combination.get(3).read_u64()?,
            0,
            children(vec![]),
        )
    } else {
        assert_eq!(combination.len()?, 3);
        let state = authenticated(value(input), &identity, "Compilation")?;
        assert_eq!(state.len()?, 5);
        assert!(child(state.clone(), 0)?.equals(&closure)?);
        let phase = child(state.clone(), 3)?.read_u64()?;
        assert!((1..=3).contains(&phase));
        (
            child(state.clone(), 1)?,
            child(state.clone(), 2)?.read_u64()?,
            phase,
            child(state, 4)?,
        )
    };
    let partitions = usize::try_from(partitions).map_err(|_| Error::OutOfBounds)?;
    assert!(partitions > 0 && partitions < u32::MAX as usize);
    if phase > 0 {
        assert_eq!(
            results.len()?,
            if phase == 2 { partitions as u64 + 1 } else { 1 }
        );
        for index in 0..usize::try_from(results.len()?)? {
            let result = child(results.clone(), index)?;
            let status = child(result.clone(), 1)?;
            assert_eq!(status.len()?, 4);
            let mut bytes = [0; 4];
            status.read_into(0, &mut bytes)?;
            if u32::from_le_bytes(bytes) != 0 {
                return Ok(value(
                    tree![tree![], result, Blob::create(b"Error")].tag(combination, 3)?,
                ));
            }
        }
    }
    let jobs = match phase {
        0 => {
            let directory = descriptor(
                "/work",
                3,
                children(vec![descriptor("module.wasm", 4, wasm.clone())]),
            );
            let arguments = ["/toolchain/usr/bin/wasm2c", "-n", "module"]
                .into_iter()
                .map(text)
                .chain([
                    text(&format!("--num-outputs={partitions}")),
                    text("/work/module.wasm"),
                    text("-o"),
                    text("/work/module.c"),
                ])
                .collect();
            vec![command(wasm2c, arguments, toolchain, directory)?]
        }
        1 => {
            let directory = work(child(results, 0)?)?;
            let mut jobs = Vec::new();
            for index in 0..=partitions {
                let source = if index == partitions {
                    text("/toolchain/share/fix/wasm-rt.c")
                } else if partitions == 1 {
                    text("/work/module.c")
                } else {
                    text(&format!("/work/module_{index}.c"))
                };
                let mut arguments: Vec<Expression<'a>> = flags();
                arguments.extend([text("-c"), source, text("-o"), text("/work/object.o")]);
                jobs.push(command(
                    clang.clone(),
                    arguments,
                    toolchain.clone(),
                    directory.clone(),
                )?);
            }
            jobs
        }
        2 => {
            let mut files = Vec::new();
            let mut arguments: Vec<Expression<'a>> = flags();
            arguments.extend(
                [
                    "-fuse-ld=/toolchain/usr/bin/ld.lld",
                    "-Wl,--build-id=none",
                    "-Wl,--threads=1",
                    "-Wl,-T,/toolchain/share/fix/memmap.ld",
                    "-o",
                    "/work/program.elf",
                ]
                .into_iter()
                .map(text),
            );
            for index in 0..=partitions {
                files.push(descriptor(
                    &format!("object_{index}.o"),
                    4,
                    output(child(results.clone(), index)?, "object.o")?,
                ));
                arguments.push(text(&format!("/work/object_{index}.o")));
            }
            arguments.push(text("/toolchain/share/fix/libfixshell.a"));
            vec![command(
                clang,
                arguments,
                toolchain,
                descriptor("/work", 3, children(files)),
            )?]
        }
        3 => {
            return Ok(value(
                tree![
                    tree![],
                    output(child(results, 0)?, "program.elf")?,
                    Blob::create(b"Runnable")
                ]
                .tag(combination, 3)?,
            ));
        }
        _ => unreachable!(),
    };
    let state = children(vec![
        closure.clone(),
        wasm,
        value(partitions as u64),
        value(phase + 1),
        children(jobs),
    ]);
    let state = value(tree![tree![], state, Blob::create(b"Compilation")].tag(combination, 3)?);
    Ok(application(vec![value(combination.get(0)), closure, state]))
}
