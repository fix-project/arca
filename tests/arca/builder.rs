#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

fn check_blob_copies() {
    let mut input = [0u8; 8193];
    for (index, byte) in input.iter_mut().enumerate() {
        *byte = index as u8;
    }
    for bytes in [&input[..], &b"valid UTF-8"[..], &b""[..]] {
        let steps = [arcane::arca_build_step {
            op: arcane::ARCA_BUILD_BLOB as u64,
            value: bytes.as_ptr() as u64,
            size: bytes.len() as u64,
        }];
        let built = unsafe { arcane::arca_build(steps.as_ptr(), steps.len()) };
        let direct = unsafe { arcane::arca_blob_create(bytes.as_ptr(), bytes.len()) };
        for descriptor in [built, direct] {
            assert!(descriptor > 0);
            let mut output = [0u8; 8193];
            assert_eq!(
                unsafe { arcane::arca_blob_read(descriptor, 0, output.as_mut_ptr(), bytes.len()) },
                bytes.len() as i64
            );
            assert_eq!(&output[..bytes.len()], bytes);
            assert_eq!(unsafe { arcane::arca_drop(descriptor) }, 0);
        }
    }
    assert!(unsafe { arcane::arca_blob_create(usize::MAX as *const u8, 1) } < 0);
    let invalid = [arcane::arca_build_step {
        op: arcane::ARCA_BUILD_BLOB as u64,
        value: u64::MAX,
        size: 1,
    }];
    assert!(unsafe { arcane::arca_build(invalid.as_ptr(), invalid.len()) } < 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    check_blob_copies();
    let blob = b"builder";
    let source = unsafe { arcane::arca_word_create(17) };
    let steps = [
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_CLONE as u64,
            value: source as u64,
            size: 0,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_BLOB as u64,
            value: blob.as_ptr() as u64,
            size: blob.len() as u64,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_TUPLE as u64,
            value: 0,
            size: 2,
        },
    ];
    let built = unsafe { arcane::arca_build(steps.as_ptr(), steps.len()) };
    assert!(built > 0);
    let first = unsafe { arcane::arca_tuple_get(built, 0) };
    let second = unsafe { arcane::arca_tuple_get(built, 1) };
    let mut number = 0;
    assert_eq!(unsafe { arcane::arca_word_read(first, &mut number) }, 8);
    assert_eq!(number, 17);
    let mut read = [0; 7];
    assert_eq!(
        unsafe { arcane::arca_blob_read(second, 0, read.as_mut_ptr(), 7) },
        7
    );
    assert_eq!(&read, blob);
    assert_eq!(unsafe { arcane::arca_word_read(source, &mut number) }, 8);
    let function_steps = [
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_BLOB as u64,
            value: b"Symbolic".as_ptr() as u64,
            size: 8,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_BLOB as u64,
            value: b"probe".as_ptr() as u64,
            size: 5,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_TUPLE as u64,
            value: 0,
            size: 2,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_FUNCTION as u64,
            value: 0,
            size: 0,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_WORD as u64,
            value: 23,
            size: 0,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_APPLY as u64,
            value: 0,
            size: 0,
        },
    ];
    let applied = unsafe { arcane::arca_build(function_steps.as_ptr(), function_steps.len()) };
    assert!(applied > 0);
    assert_eq!(
        unsafe { arcane::arca_type(applied) },
        arcane::__TYPE_function as i64
    );
    let invalid = [
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_CLONE as u64,
            value: source as u64,
            size: 0,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_WORD as u64,
            value: 3,
            size: 0,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_APPLY as u64,
            value: 0,
            size: 0,
        },
    ];
    assert_eq!(
        unsafe { arcane::arca_build(invalid.as_ptr(), invalid.len()) },
        -(arcane::__ERR_bad_type as i64)
    );
    assert_eq!(unsafe { arcane::arca_word_read(source, &mut number) }, 8);
    let takes = [
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_TAKE as u64,
            value: source as u64,
            size: 0,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_TAKE as u64,
            value: source as u64,
            size: 0,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_TUPLE as u64,
            value: 0,
            size: 2,
        },
    ];
    let taken = unsafe { arcane::arca_build(takes.as_ptr(), takes.len()) };
    assert!(taken > 0);
    let taken_word = unsafe { arcane::arca_tuple_get(taken, 0) };
    assert_eq!(
        unsafe { arcane::arca_word_read(taken_word, &mut number) },
        8
    );
    assert_eq!(number, 17);
    let emptied = unsafe { arcane::arca_tuple_get(taken, 1) };
    assert_eq!(
        unsafe { arcane::arca_type(emptied) },
        arcane::__TYPE_null as i64
    );
    assert_eq!(unsafe { arcane::arca_word_read(first, &mut number) }, 8);
    assert_eq!(number, 17);
    for descriptor in [built, first, second, taken, taken_word, emptied] {
        assert_eq!(unsafe { arcane::arca_drop(descriptor) }, 0);
    }
    let call = [
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_TAKE as u64,
            value: applied as u64,
            size: 0,
        },
        arcane::arca_build_step {
            op: arcane::ARCA_BUILD_CALL as u64,
            value: 0,
            size: 0,
        },
    ];
    assert!(unsafe { arcane::arca_build(call.as_ptr(), call.len()) } >= 0);
    let reply: Word = os::argument().try_into().unwrap();
    assert_eq!(reply.read(), 37);
    os::exit(Word::new(1));
}
