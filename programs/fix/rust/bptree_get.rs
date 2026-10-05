#![no_std]
#![feature(asm_experimental_arch)]
extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use fixutils::*;

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

fn upper_bound(keys: &[i32], key: i32) -> usize {
    keys.partition_point(|&k| k <= key)
}

#[procedure_entrypoint]
pub fn _fixpoint_apply(combination: Combination) -> Result<HandleOp<'static>, Error> {
    let arguments = combination.to_entries()?;
    let [_, keys_h, childrenordata_h, key_h] = arguments[..] else {
        panic!("expected: keys, children or data, key");
    };

    let key = i32::from_le_bytes(key_h.to_bytes()?.try_into().unwrap());
    let keys_data = keys_h.to_bytes()?;
    let isleaf = keys_data[0] != 0;
    let (key_bytes, _) = keys_data[1..].as_chunks();
    let keys: Vec<i32> = key_bytes.iter().copied().map(i32::from_le_bytes).collect();

    let idx = upper_bound(&keys, key);
    let selection_table = Table::with_capacity(2)?;
    unsafe {
        if isleaf {
            if idx != 0 && keys[idx - 1] == key {
                selection_table.set(0, childrenordata_h);
                selection_table.set(1, from_bytes(&(idx as u64).to_le_bytes())?);
                Ok(HandleOp::Selection(Box::leak(Box::new(
                    selection_table.to_handle(2)?.into(),
                ))))
            } else {
                Ok(selection_table.to_handle(0)?.into())
            }
        } else {
            let call_table = Table::with_capacity(4)?;

            selection_table.set(0, childrenordata_h);
            selection_table.set(1, from_bytes(&(idx as u64 + 1).to_le_bytes())?);
            call_table.set(2, ShallowEncode(Selection(selection_table.to_handle(2)?)));

            selection_table.set(0, call_table.get(2));
            selection_table.set(1, from_bytes(&0u64.to_le_bytes())?);
            call_table.set(1, StrictEncode(Selection(selection_table.to_handle(2)?)));

            call_table.set(0, arguments[0]);
            call_table.set(3, key_h);

            Ok(HandleOp::Application(Box::leak(Box::new(
                call_table.to_handle(4)?.into(),
            ))))
        }
    }
}
