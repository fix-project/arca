use crate::prelude::*;
use common::elfloader;

const PROGRAM: &[u8] = include_bytes!(env!("ARCA_SIMD_PROGRAM"));

fn suspend(byte: u8) -> Function {
    let program: Function = elfloader::load_elf(PROGRAM).unwrap();
    program
        .apply(Blob::new([byte; 32]))
        .force()
        .try_into()
        .unwrap()
}

fn resume(function: Function, byte: u8) {
    let result: Blob = function.apply(None).force().try_into().unwrap();
    assert_eq!(result.len(), 1042);
    assert_eq!(&result[..512], &[0; 512]);
    assert_eq!(&result[512..516], &0x1f80u32.to_le_bytes());
    assert_eq!(&result[516..518], &0x037fu16.to_le_bytes());
    for index in 0..16 {
        assert_eq!(
            &result[518 + 32 * index..550 + 32 * index],
            &[byte + index as u8; 32]
        );
    }
    assert_eq!(&result[1030..1034], &0x3f80u32.to_le_bytes());
    assert_eq!(&result[1034..], &1.0f64.to_le_bytes());
}

/// Preserves all YMM registers, MXCSR, and x87 state across syscalls, suspension,
/// interleaved processes, cloning, and function serialization.
#[test]
fn test_simd_continuations() {
    let first = suspend(28);
    let second = suspend(3);
    let clone = first.clone();
    let definition = first.read();
    let restored = Function::new(definition.clone()).unwrap();
    let definition: Tuple = definition.try_into().unwrap();
    let expected: Tuple = definition.get(1).try_into().unwrap();
    let definition: Tuple = restored.read_cloned().try_into().unwrap();
    let actual: Tuple = definition.get(1).try_into().unwrap();
    assert_eq!(actual.get(3), expected.get(3));
    assert_eq!(actual.get(4), expected.get(4));
    resume(second, 3);
    resume(restored, 28);
    resume(clone, 28);
}

/// Exercises explicit unloading as well as the swap used to capture continuations.
#[test]
fn test_simd_load_unload() {
    let mut function = suspend(91).into_inner();
    let process = function.arca_mut().unwrap().clone();
    let expected = process.clone();
    let mut cpu = CPU.borrow_mut();
    let process = process.load(&mut cpu).unload();
    let (registers, _, _) = process.read();
    let (expected_registers, _, _) = expected.read();
    assert_eq!(registers, expected_registers);
}
