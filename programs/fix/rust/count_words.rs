#![no_std]
#![feature(asm_experimental_arch)]
extern crate alloc;

use fixutils::*;

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[procedure_entrypoint]
pub fn _fixpoint_apply(combination: Combination) -> Result<HandleOp<'static>, Error> {
    let arguments = combination.to_entries()?;
    let [_, tuple] = arguments[..] else {
        return Ok(from_bytes(&u64::MAX.to_le_bytes())?.into());
    };
    let tuple = tuple.to_entries()?;
    let [needle, haystack] = tuple[..] else {
        return Ok(from_bytes(&u64::MAX.to_le_bytes())?.into());
    };

    let needle = needle.to_bytes()?;
    let haystack = haystack.to_bytes()?;
    let count = if needle.is_empty() {
        0
    } else {
        haystack
            .windows(needle.len())
            .filter(|&window| window == needle)
            .count()
    };
    Ok(from_bytes(&(count as u64).to_le_bytes())?.into())
}
