#![no_std]
#![feature(asm_experimental_arch)]
extern crate alloc;

use alloc::boxed::Box;
use fixutils::*;

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[procedure_entrypoint]
pub fn _fixpoint_apply(combination: Combination) -> Result<HandleOp<'static>, Error> {
    let arguments = combination.to_entries()?;
    let [fib, n, add] = arguments[..] else {
        panic!("expected: n, add");
    };

    let n = u64::from_le_bytes(n.to_bytes()?.try_into().unwrap());
    if n <= 1 {
        return Ok(from_bytes(&1u64.to_le_bytes())?.into());
    }

    let first_call_table = Table::with_capacity(3)?;
    let second_call_table = Table::with_capacity(3)?;
    let addition_table = Table::with_capacity(3)?;
    unsafe {
        first_call_table.set(0, fib);
        first_call_table.set(1, from_bytes(&(n - 1).to_le_bytes())?);
        first_call_table.set(2, add);

        second_call_table.set(0, fib);
        second_call_table.set(1, from_bytes(&(n - 2).to_le_bytes())?);
        second_call_table.set(2, add);

        addition_table.set(0, add);
        addition_table.set(1, StrictEncode(Application(first_call_table.to_handle(3)?)));
        addition_table.set(
            2,
            StrictEncode(Application(second_call_table.to_handle(3)?)),
        );
    }

    Ok(HandleOp::Application(Box::leak(Box::new(
        addition_table.to_handle(3)?.into(),
    ))))
}
