#![no_std]
extern crate alloc;

use alloc::{collections::BTreeMap, format, string::String, vec, vec::Vec};
use fixwasm::{Any, Blob, Error, Handle, Object, Thunk, Tree, Value, tree};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

type Expression = Handle<Any>;

/// Wasm bytes per C translation unit when the request names no unit count.
const UNIT_BYTES: u64 = 128 << 10;

fn value(value: impl Value) -> Expression {
    value.erase()
}
fn text(text: &str) -> Expression {
    value(Blob::create(text.as_bytes().to_vec()))
}
fn children(values: Vec<Expression>) -> Expression {
    value(Tree::from_iter(values))
}
fn child(value: Expression, index: usize) -> Result<Expression, Error> {
    Ok(value.get(index))
}
fn application(arguments: Vec<Expression>) -> Expression {
    value(Tree::from_iter(arguments).application())
}
fn strict(thunk: Expression) -> Result<Expression, Error> {
    let thunk: Handle<Thunk> = thunk.try_into()?;
    Ok(value(thunk.strict()))
}
fn shallow(thunk: Expression) -> Result<Expression, Error> {
    let thunk: Handle<Thunk> = thunk.try_into()?;
    Ok(value(thunk.shallow()))
}
fn select(input: Expression, index: u64) -> Expression {
    value(tree![input, index].selection())
}
fn stat(filetype: u8) -> Expression {
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
fn descriptor(name: &str, filetype: u8, contents: Expression) -> Expression {
    children(vec![text(name), stat(filetype), contents])
}
fn command(
    memory: u64,
    executable: Expression,
    arguments: Vec<Expression>,
    toolchain: Expression,
    directory: Expression,
) -> Result<Expression, Error> {
    let stream = |name| value(tree![Blob::create(name), tree![], Blob::create(b"")]);
    let application = application(vec![
        value(memory),
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
    ]);
    fixwasm::advise(&application, fixwasm::advice::CANONICALIZE, b"")?;
    let result = shallow(value(application.capture()?))?;
    let filesystem = shallow(select(result.clone(), 0))?;
    let step = children(vec![
        strict(select(filesystem, 4))?,
        strict(select(result.clone(), 1))?,
        result,
    ]);
    fixwasm::advise(&step, fixwasm::advice::CANONICALIZE, b"")?;
    Ok(step)
}
fn work(result: Expression) -> Result<Expression, Error> {
    child(result, 0)
}
fn output(result: Expression, name: &str) -> Result<Expression, Error> {
    let files = child(work(result)?, 2)?;
    for index in 0..usize::try_from(files.len()?)? {
        let file = child(files.clone(), index)?;
        if child(file.clone(), 0)?.equals(&text(name))? {
            return child(file, 2);
        }
    }
    Err(Error::OutOfBounds)
}
fn flags() -> Vec<Expression> {
    [
        "/toolchain/usr/bin/clang",
        "--target=x86_64-unknown-linux-gnu",
        "-O2",
        "-fno-optimize-sibling-calls",
        "-frounding-math",
        "-ffreestanding",
        "-fno-stack-protector",
        "-nostdlib",
        "-nostdinc",
        "-isystem",
        "/toolchain/usr/lib/clang/23/include",
        "-isystem",
        "/toolchain/usr/include/wasm32-wasip1",
        "-isystem",
        "/toolchain/usr/include",
        "-mcmodel=small",
        "-fPIC",
        "-fvisibility=hidden",
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
fn authenticated(tag: Expression, author: &Expression, claim: &str) -> Result<Expression, Error> {
    assert!(tag.is_tag()? && tag.len()? == 3);
    assert!(child(tag.clone(), 2)?.equals(&text(claim))?);
    assert!(child(tag.clone(), 0)?.equals(author)?);
    child(tag, 1)
}

fn memory_limit(explicit: u64, bytes: u64, base: u64, factor: u64) -> Result<u64, Error> {
    if explicit != 0 {
        return Ok(explicit);
    }
    bytes
        .checked_mul(factor)
        .and_then(|scaled| scaled.checked_add(base))
        .ok_or(Error::OutOfBounds)
}

fn clang_compile_memory(explicit: u64, bytes: u64) -> Result<u64, Error> {
    let smaller = bytes.min(16 << 20);
    memory_limit(explicit, bytes - smaller, (768 << 20) + 64 * smaller, 128)
}

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<Expression, Error> {
    fixwasm::advise(combination, fixwasm::advice::CANONICALIZE, b"")?;
    fixwasm::advise(combination, fixwasm::advice::TITLE, b"compile Wasm")?;
    let closure = value(combination.get(1));
    let identity =
        value(Handle::<Object<Any>>::try_from(child(child(closure.clone(), 1)?, 1)?)?.reference());
    let resources = authenticated(child(closure.clone(), 2)?, &identity, "Bootstrap")?;
    let wasm2c = child(resources.clone(), 0)?;
    let clang = child(resources.clone(), 1)?;
    let toolchain = child(resources, 2)?;
    let input = combination.get(2);
    let (wasm, partitions, phase, results, wasm2c_memory, clang_memory) = if input.value_type()?
        == fixwasm::ValueType::Object
        && input.data_type()? == fixwasm::DataType::Blob
    {
        let mut next = 3;
        let mut partitions = input.len()?.div_ceil(UNIT_BYTES).max(1);
        if next < combination.len()? as usize
            && combination.get(next).data_type()? == fixwasm::DataType::Blob
        {
            partitions = combination.get(next).read_u64()?;
            next += 1;
        }
        let (wasm2c_memory, clang_memory) = if next < combination.len()? as usize {
            let budgets = combination.get(next);
            assert_eq!(budgets.len()?, 2);
            next += 1;
            (budgets.get(0).read_u64()?, budgets.get(1).read_u64()?)
        } else {
            (0, 0)
        };
        assert_eq!(combination.len()?, next as u64);
        (
            value(input),
            partitions,
            0,
            children(vec![]),
            wasm2c_memory,
            clang_memory,
        )
    } else {
        assert_eq!(combination.len()?, 3);
        let state = authenticated(value(input), &identity, "Compilation")?;
        assert_eq!(state.len()?, 7);
        assert!(child(state.clone(), 0)?.equals(&closure)?);
        let phase = child(state.clone(), 3)?.read_u64()?;
        assert!((1..=3).contains(&phase));
        (
            child(state.clone(), 1)?,
            child(state.clone(), 2)?.read_u64()?,
            phase,
            child(state.clone(), 4)?,
            child(state.clone(), 5)?.read_u64()?,
            child(state, 6)?.read_u64()?,
        )
    };
    fixwasm::advise(
        combination,
        fixwasm::advice::TITLE,
        match phase {
            0 => b"compile Wasm: translating Wasm" as &[u8],
            1 => b"compile Wasm: compiling C",
            2 => b"compile Wasm: linking",
            3 => b"compile Wasm: issuing Runnable",
            _ => unreachable!(),
        },
    )?;
    let partitions = usize::try_from(partitions).map_err(|_| Error::OutOfBounds)?;
    assert!(partitions > 0 && partitions < u32::MAX as usize);
    let object_count = partitions.checked_add(1).ok_or(Error::OutOfBounds)?;
    if phase > 0 {
        assert_eq!(
            results.len()?,
            if phase == 2 { object_count as u64 } else { 1 }
        );
        for index in 0..usize::try_from(results.len()?)? {
            let result = child(results.clone(), index)?;
            let status = child(result.clone(), 1)?;
            let mut bytes = [0; 4];
            let exit_status =
                if status.data_type()? == fixwasm::DataType::Blob && status.len()? == 4 {
                    status.read_into(0, &mut bytes)?;
                    Some(u32::from_le_bytes(bytes))
                } else {
                    None
                };
            if child(result.clone(), 0)?.data_type()? != fixwasm::DataType::Tree
                || exit_status != Some(0)
            {
                let original = strict(value(child(result, 2)?.identification()))?;
                return Ok(value(
                    tree![identity.clone(), original, Blob::create(b"Error")].tag()?,
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
            let arguments = [
                "/toolchain/usr/bin/wasm2c",
                "--enable-threads",
                "--enable-custom-page-sizes",
                "--enable-compact-imports",
                "-n",
                "module",
            ]
            .into_iter()
            .map(text)
            .chain([
                text(&format!("--num-outputs={partitions}")),
                text("/work/module.wasm"),
                text("-o"),
                text("/work/module.c"),
            ])
            .collect();
            let memory = if wasm2c_memory == 0 {
                memory_limit(0, wasm.len()?, 4 << 30, 45)?
                    .checked_next_multiple_of(256 << 20)
                    .ok_or(Error::OutOfBounds)?
            } else {
                wasm2c_memory
            };
            vec![command(memory, wasm2c, arguments, toolchain, directory)?]
        }
        1 => {
            let directory = work(child(results, 0)?)?;
            let files = child(directory.clone(), 2)?;
            let mut by_name = BTreeMap::new();
            for index in 0..usize::try_from(files.len()?)? {
                let file = child(files.clone(), index)?;
                let name = child(file.clone(), 0)?.read()?;
                by_name.entry(name).or_insert(file);
            }
            let headers: Vec<_> = [b"module.h".as_slice(), b"module-impl.h".as_slice()]
                .into_iter()
                .filter_map(|name| by_name.remove(name))
                .collect();
            let mut jobs = Vec::new();
            for index in 0..object_count {
                let (source, bytes, files) = if index == partitions {
                    (text("/toolchain/share/fix/wasm-rt.c"), 0, headers.clone())
                } else {
                    let name = if partitions == 1 {
                        String::from("module.c")
                    } else {
                        format!("module_{index}.c")
                    };
                    let file = by_name.remove(name.as_bytes()).ok_or(Error::OutOfBounds)?;
                    let bytes = child(file.clone(), 2)?.len()?;
                    let mut files = headers.clone();
                    files.push(file);
                    (text(&format!("/work/{name}")), bytes, files)
                };
                let mut arguments: Vec<Expression> = flags();
                if index == partitions {
                    arguments.push(text("-O3"));
                }
                arguments.extend([text("-c"), source, text("-o"), text("/work/object.o")]);
                jobs.push(command(
                    clang_compile_memory(clang_memory, bytes)?,
                    clang.clone(),
                    arguments,
                    toolchain.clone(),
                    descriptor("/work", 3, children(files)),
                )?);
            }
            jobs
        }
        2 => {
            let mut files = Vec::new();
            let mut bytes = 0u64;
            let mut arguments: Vec<Expression> = flags();
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
            for index in 0..object_count {
                let object = output(child(results.clone(), index)?, "object.o")?;
                bytes = bytes.checked_add(object.len()?).ok_or(Error::OutOfBounds)?;
                files.push(descriptor(&format!("object_{index}.o"), 4, object));
                arguments.push(text(&format!("/work/object_{index}.o")));
            }
            arguments.extend(["/toolchain/share/fix/libfixshell.a"].into_iter().map(text));
            vec![command(
                memory_limit(clang_memory, bytes, 1 << 30, 12)?,
                clang,
                arguments,
                toolchain,
                descriptor("/work", 3, children(files)),
            )?]
        }
        3 => {
            return Ok(value(
                tree![
                    identity,
                    output(child(results, 0)?, "program.elf")?,
                    Blob::create(b"Runnable")
                ]
                .tag()?,
            ));
        }
        _ => unreachable!(),
    };
    let output_bytes = wasm.len()?;
    let parallel = if phase == 0 {
        object_count as u64
    } else {
        1u64
    };
    let state = children(vec![
        closure.clone(),
        wasm,
        value(partitions as u64),
        value(phase + 1),
        children(jobs),
        value(wasm2c_memory),
        value(clang_memory),
    ]);
    let state = value(tree![identity, state, Blob::create(b"Compilation")].tag()?);
    let next = Tree::from_iter([value(combination.get(0)), closure, state]);
    fixwasm::advise(&next, "fix::output-size", &output_bytes.to_le_bytes())?;
    fixwasm::advise(&next, "fix::parallel", &parallel.to_le_bytes())?;
    Ok(value(next.application()))
}
