#![no_std]
#![feature(asm_experimental_arch)]
extern crate alloc;

use fixutils::*;

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

struct MapReduce<'a> {
    mapper: TableGet<'a>,
    reducer: TableGet<'a>,
    target: TableGet<'a>,

    result_table: &'a mut Table,
    call_table: &'a mut Table,
    selection_table: &'a mut Table,
    index_memory: &'a mut Memory,
}

impl MapReduce<'_> {
    fn mapreduce(&mut self, start: usize, end: usize, result_index: usize) -> Result<(), Error> {
        unsafe {
            if end - start <= 1 {
                self.index_memory.write(&(start as u64).to_le_bytes());
                self.selection_table.set(0, self.target);
                self.selection_table.set(1, self.index_memory.to_handle(8)?);
                self.call_table.set(0, self.mapper);
                self.call_table.set(
                    1,
                    StrictEncode(Selection(self.selection_table.to_handle(2)?)),
                );
                self.result_table
                    .set(result_index, Application(self.call_table.to_handle(2)?));
            } else {
                let split = start + (end - start) / 2;
                self.mapreduce(start, split, result_index)?;
                self.mapreduce(split, end, result_index + 1)?;
                self.call_table.set(0, self.reducer);
                self.call_table
                    .set(1, StrictEncode(self.result_table.get(result_index)));
                self.call_table
                    .set(2, StrictEncode(self.result_table.get(result_index + 1)));
                self.result_table
                    .set(result_index, Application(self.call_table.to_handle(3)?));
            }
        }
        Ok(())
    }
}

// Takes (mapper, reducer, tree) combination
#[procedure_entrypoint]
pub fn _fixpoint_apply(combination: Combination) -> Result<HandleOp<'static>, Error> {
    let arguments = combination.to_entries()?;
    let [_, mapper, reducer, target] = arguments[..] else {
        panic!("expected: mapper, reducer, target");
    };

    let n = target.len();
    let result_table = Table::with_capacity(n)?;
    MapReduce {
        mapper,
        reducer,
        target,
        result_table,
        call_table: Table::with_capacity(3)?,
        selection_table: Table::with_capacity(2)?,
        index_memory: Memory::with_capacity(8)?,
    }
    .mapreduce(0, n, 0)?;
    Ok(unsafe { result_table.get(0) }.into())
}
