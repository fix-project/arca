use alloc::{collections::BTreeMap, vec::Vec};
use arca::Value;
use user::Runtime;

use crate::handles::{self, Query, Reference};

struct Object {
    backing: Value<Runtime>,
    dependencies: Vec<usize>,
}

static mut OBJECTS: BTreeMap<usize, Object> = BTreeMap::new();
static mut CELLS: Vec<usize> = Vec::new();

pub unsafe fn create(
    result: Reference,
    backing: Value<Runtime>,
    bytes: usize,
    metadata: u16,
    dependencies: Vec<usize>,
) {
    let cell = handles::allocate();
    unsafe { handles::create_local(cell, bytes, metadata) };
    unsafe {
        (&mut *(&raw mut OBJECTS)).insert(
            cell,
            Object {
                backing,
                dependencies,
            },
        );
        (&mut *(&raw mut CELLS)).push(cell);
    }
    unsafe { handles::copy_handle(cell, handles::address(result), 0) };
}

pub fn lift(roots: &[Reference]) {
    if roots
        .iter()
        .all(|root| unsafe { handles::query(*root, Query::Local) } == 0)
    {
        return;
    }
    let roots: Vec<_> = roots.iter().map(|root| handles::address(*root)).collect();
    lift_cells(roots, Vec::new());
}

pub fn lift_table(address: usize, length: usize) {
    lift_cells(Vec::new(), alloc::vec![(address, length)]);
}

pub fn retain(roots: &[Reference]) {
    if unsafe { (&*(&raw const CELLS)).is_empty() } {
        return;
    }
    let roots = roots.iter().map(|root| handles::address(*root)).collect();
    let mut tables = alloc::vec![crate::references::range()];
    unsafe {
        for index in 0..crate::rt::TABLE_IDX {
            let table = crate::rt::TABLES[index];
            if !table.is_null() && !crate::rt::TABLE_ATTACHED[index] {
                tables.push(((*table).data as usize, (*table).size as usize));
            }
        }
    }
    lift_cells(roots, tables);
    unsafe {
        (&mut *(&raw mut OBJECTS)).clear();
        (&mut *(&raw mut CELLS)).clear();
    }
}

fn lift_cells(roots: Vec<usize>, tables: Vec<(usize, usize)>) {
    let mut pending = Vec::new();
    for cell in roots
        .iter()
        .copied()
        .chain(tables.iter().flat_map(|&(address, length)| {
            (0..length).map(move |index| address + index * fixhandle::HANDLE_SIZE)
        }))
    {
        let source = unsafe { handles::query_cell(cell, Query::LocalAddress) };
        if source != 0 {
            pending.push(source);
        }
    }
    if pending.is_empty() {
        return;
    }
    let mut selected = Vec::new();
    let objects = unsafe { &mut *(&raw mut OBJECTS) };
    while let Some(cell) = pending.pop() {
        if let Some(mut object) = objects.remove(&cell) {
            pending.append(&mut object.dependencies);
            selected.push((cell, object));
        }
    }
    send(selected, roots, tables);
}

pub fn lift_all() {
    let objects = unsafe { core::mem::take(&mut *(&raw mut OBJECTS)) };
    let cells = unsafe { core::mem::take(&mut *(&raw mut CELLS)) };
    if cells.is_empty() {
        return;
    }
    let mut tables = alloc::vec![crate::references::range()];
    unsafe {
        for index in 0..crate::rt::TABLE_IDX {
            let table = crate::rt::TABLES[index];
            if !table.is_null() && !crate::rt::TABLE_ATTACHED[index] {
                tables.push(((*table).data as usize, (*table).size as usize));
            }
        }
    }
    send(objects.into_iter().collect(), cells, tables);
}

fn send(objects: Vec<(usize, Object)>, cells: Vec<usize>, tables: Vec<(usize, usize)>) {
    use crate::continuation::Argument::{Tuple, Value, Word};
    let records = Tuple(
        objects
            .into_iter()
            .map(|(cell, object)| Tuple(alloc::vec![Word(cell as u64), Value(object.backing)]))
            .collect(),
    );
    let aliases = Tuple(
        cells
            .into_iter()
            .rev()
            .map(|cell| Word(cell as u64))
            .collect(),
    );
    let mappings = Tuple(
        tables
            .into_iter()
            .map(|(address, length)| Tuple(alloc::vec![Word(address as u64), Word(length as u64)]))
            .collect(),
    );
    crate::continuation::operation("lift")
        .argument(records)
        .argument(aliases)
        .argument(mappings)
        .call_with_current_continuation();
}
