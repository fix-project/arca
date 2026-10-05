#![no_std]
#![feature(asm_experimental_arch)]
extern crate alloc;

use fixutils::*;

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

fn map(
    main_blob: TableGet,
    arr: &[TableGet],
    arg_1: TableGet,
) -> Result<CreateTree<'static>, Error> {
    let result_table = Table::with_capacity(arr.len())?;
    let call_table = Table::with_capacity(3)?;
    unsafe {
        call_table.set(0, main_blob);
        call_table.set(1, arg_1);
        for (i, &element) in arr.iter().enumerate() {
            call_table.set(2, element);
            result_table.set(i, Application(call_table.to_handle(3)?));
        }
    }
    result_table.to_handle(arr.len())
}

// Takes (function, argument, tree) combination and returns a tree of thunks applying the function to argument and elements
#[procedure_entrypoint]
pub fn _fixpoint_apply(combination: Combination) -> Result<HandleOp<'static>, Error> {
    let arguments = combination.to_entries()?;
    let [_, main_blob, arg_1, arr] = arguments[..] else {
        panic!("expected: function, argument, tree");
    };
    Ok(map(main_blob, &arr.to_entries()?, arg_1)?.into())
}
