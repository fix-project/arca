use crate::handles::{self, Reference};
use arca::{Blob, Function, Tuple, Value, Word};
use user::Runtime;

use crate::rt::{HANDLE_BASE, SLOT_SIZE};

static mut OPERATIONS: alloc::collections::BTreeMap<&'static str, Function<Runtime>> =
    alloc::collections::BTreeMap::new();

fn step(op: u32, value: u64, size: u64) -> arcane::arca_build_step {
    arcane::arca_build_step {
        op: op as u64,
        value,
        size,
    }
}

pub enum Argument {
    Word(u64),
    Blob(alloc::vec::Vec<u8>),
    Value(Value<Runtime>),
    Tuple(alloc::vec::Vec<Argument>),
}

impl Argument {
    fn steps(&self) -> usize {
        match self {
            Self::Tuple(fields) => 1 + fields.iter().map(Self::steps).sum::<usize>(),
            _ => 1,
        }
    }

    fn materialize(self) -> Value<Runtime> {
        if self.steps() > 4096 {
            let Self::Tuple(fields) = self else {
                unreachable!()
            };
            let mut tuple = Tuple::<Runtime>::new(fields.len());
            for (index, field) in fields.into_iter().enumerate() {
                tuple.set(index, field.materialize());
            }
            return tuple.into();
        }
        let mut request = Request {
            steps: alloc::vec::Vec::new(),
            owned: alloc::vec::Vec::new(),
            blobs: alloc::vec::Vec::new(),
            bytes: core::marker::PhantomData,
        };
        request.push(self);
        for value in request.owned.drain(..) {
            value.into_raw();
        }
        user::build(&request.steps).unwrap()
    }
}

pub struct Request<'a> {
    steps: alloc::vec::Vec<arcane::arca_build_step>,
    owned: alloc::vec::Vec<user::Ref>,
    blobs: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    bytes: core::marker::PhantomData<&'a [u8]>,
}

pub fn operation<'a>(name: &'static str) -> Request<'a> {
    let base = unsafe {
        (&mut *(&raw mut OPERATIONS))
            .entry(name)
            .or_insert_with(|| {
                let steps = [
                    step(arcane::ARCA_BUILD_BLOB, b"Symbolic".as_ptr() as u64, 8),
                    step(
                        arcane::ARCA_BUILD_BLOB,
                        name.as_ptr() as u64,
                        name.len() as u64,
                    ),
                    step(arcane::ARCA_BUILD_TUPLE, 0, 2),
                    step(arcane::ARCA_BUILD_FUNCTION, 0, 0),
                ];
                user::build(&steps).unwrap().try_into().unwrap()
            })
            .inner()
            .as_raw()
    };
    Request {
        steps: alloc::vec![step(arcane::ARCA_BUILD_CLONE, base as u64, 0)],
        owned: alloc::vec::Vec::new(),
        blobs: alloc::vec::Vec::new(),
        bytes: core::marker::PhantomData,
    }
}

impl<'a> Request<'a> {
    fn push(&mut self, argument: Argument) {
        match argument {
            Argument::Word(value) => self.steps.push(step(arcane::ARCA_BUILD_WORD, value, 0)),
            Argument::Blob(bytes) => {
                self.steps.push(step(
                    arcane::ARCA_BUILD_BLOB,
                    bytes.as_ptr() as u64,
                    bytes.len() as u64,
                ));
                self.blobs.push(bytes);
            }
            Argument::Value(value) => {
                use arca::RawValue;
                let value = match value.into_inner() {
                    RawValue::Null(x)
                    | RawValue::Word(x)
                    | RawValue::Blob(x)
                    | RawValue::Tuple(x)
                    | RawValue::Page(x)
                    | RawValue::Table(x)
                    | RawValue::Function(x) => x,
                };
                self.steps
                    .push(step(arcane::ARCA_BUILD_TAKE, value.as_raw() as u64, 0));
                self.owned.push(value);
            }
            Argument::Tuple(fields) => {
                let count = fields.len();
                for field in fields {
                    self.push(field);
                }
                self.steps
                    .push(step(arcane::ARCA_BUILD_TUPLE, 0, count as u64));
            }
        }
    }

    fn reserve(&mut self, count: usize) {
        if self.steps.len() + count + 1 <= 4096 {
            return;
        }
        for value in self.owned.drain(..) {
            value.into_raw();
        }
        let value = user::build(&self.steps).unwrap();
        self.steps.clear();
        self.blobs.clear();
        self.push(Argument::Value(value));
    }

    pub fn argument(mut self, argument: Argument) -> Self {
        let argument = if argument.steps() + 3 > 4096 {
            Argument::Value(argument.materialize())
        } else {
            argument
        };
        self.reserve(argument.steps() + 1);
        self.push(argument);
        self.steps.push(step(arcane::ARCA_BUILD_APPLY, 0, 0));
        self
    }

    pub fn word(mut self, value: u64) -> Self {
        self.reserve(2);
        self.steps.push(step(arcane::ARCA_BUILD_WORD, value, 0));
        self.steps.push(step(arcane::ARCA_BUILD_APPLY, 0, 0));
        self
    }
    pub fn blob(mut self, bytes: &'a [u8]) -> Self {
        self.reserve(2);
        self.steps.push(step(
            arcane::ARCA_BUILD_BLOB,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
        ));
        self.steps.push(step(arcane::ARCA_BUILD_APPLY, 0, 0));
        self
    }
    pub fn apply(self, value: impl Into<Value<Runtime>>) -> Self {
        self.argument(Argument::Value(value.into()))
    }
    pub fn call_with_current_continuation(mut self) -> Value<Runtime> {
        self.steps.push(step(arcane::ARCA_BUILD_CALL, 0, 0));
        for value in self.owned.drain(..) {
            value.into_raw();
        }
        assert!(unsafe { arcane::arca_build(self.steps.as_ptr(), self.steps.len()) } >= 0);
        user::os::argument()
    }
}

static mut USED: usize = 2 * fixhandle::HANDLE_SIZE;
const SCRATCH: usize = crate::references::BASE - 4096;

pub fn reserve(bytes: usize, alignment: usize) -> usize {
    unsafe {
        let offset = USED.next_multiple_of(alignment);
        USED = offset.checked_add(bytes).expect("handle region overflow");
        assert!(
            USED <= crate::references::BASE - HANDLE_BASE - 4096,
            "handle region exhausted"
        );
        HANDLE_BASE + offset
    }
}

pub fn invalidate_externrefs(keep: Reference) -> Reference {
    use crate::rt;
    let mut roots = alloc::vec![keep];
    unsafe {
        for index in 0..rt::MEMORY_IDX {
            if rt::MEMORY_ATTACHED[index] {
                roots.push(rt::MEMORY_HANDLES[index]);
            }
        }
        for index in 0..rt::TABLE_IDX {
            if rt::TABLE_ATTACHED[index] {
                roots.push(rt::TABLE_HANDLES[index]);
            }
        }
    }
    crate::locals::retain(&roots);
    assert!(roots.len() * fixhandle::HANDLE_SIZE <= 4096);
    unsafe {
        rt::map_region(SCRATCH as *mut u8, 4096, arcane::__MODE_read_write)
            .expect("root allocation failed");
        for (index, root) in roots.iter().enumerate() {
            handles::copy_handle(handles::address(*root), SCRATCH + index * 32, 0);
        }
        rt::map_region(
            HANDLE_BASE as *mut u8,
            USED.next_multiple_of(4096),
            arcane::__MODE_none,
        )
        .expect("arena unmap failed");
        USED = fixhandle::HANDLE_SIZE;
        rt::map_region(HANDLE_BASE as *mut u8, 4096, arcane::__MODE_read_write)
            .expect("arena allocation failed");
        for (index, root) in roots.iter_mut().enumerate() {
            let cell = handles::allocate();
            handles::copy_handle(SCRATCH + index * 32, cell, 0);
            *root = handles::from_cell(cell);
        }
        rt::map_region(SCRATCH as *mut u8, 4096, arcane::__MODE_none).expect("root unmap failed");
        let mut next = 1;
        for index in 0..rt::MEMORY_IDX {
            rt::MEMORY_HANDLES[index] = if rt::MEMORY_ATTACHED[index] {
                let root = roots[next];
                next += 1;
                root
            } else {
                core::ptr::null_mut()
            };
        }
        for index in 0..rt::TABLE_IDX {
            rt::TABLE_HANDLES[index] = if rt::TABLE_ATTACHED[index] {
                let root = roots[next];
                next += 1;
                root
            } else {
                core::ptr::null_mut()
            };
        }
    }
    roots[0]
}

pub fn receive(destination: Reference, function: Request<'_>) {
    let address = handles::address(destination);
    let result = function
        .word(address as u64)
        .call_with_current_continuation();
    drop(result);
}

pub fn read_reply(reply: Value<Runtime>) -> Reference {
    let blob = Blob::<Runtime>::try_from(reply).expect("expected a local handle");
    let mut bytes = [0; 8];
    assert_eq!(blob.len(), bytes.len());
    assert_eq!(blob.read(0, &mut bytes), bytes.len());
    u64::from_le_bytes(bytes) as usize as Reference
}

pub fn transfer(destination: Reference, name: &'static str, value: Reference) {
    crate::locals::lift(&[value]);
    let bytes = handles::bytes(value);
    let result = operation(name)
        .blob(&bytes)
        .word(handles::address(destination) as u64)
        .call_with_current_continuation();
    let result = if matches!(result, Value::Null(_)) {
        crate::locals::lift_all();
        crate::shell::clear_privilege_cache();
        operation("capture")
            .word(unsafe { USED } as u64)
            .argument(attachments())
            .call_with_current_continuation()
    } else {
        result
    };
    drop(result);
}

pub fn advise(value: Reference, key: &[u8], payload: &[u8]) {
    crate::locals::lift(&[value]);
    operation("advise")
        .blob(&handles::bytes(value))
        .blob(key)
        .blob(payload)
        .call_with_current_continuation();
}

pub fn coordinate(destination: Reference, name: &[u8], table: u32, count: u32) -> u64 {
    assert!(!unsafe { crate::rt::TABLE_ATTACHED[table as usize] });
    let table = unsafe { &*crate::rt::externref_table(table) };
    assert!(count <= table.size);
    let data = table.data as usize;
    crate::locals::lift_table(data, count as usize);
    let bytes = unsafe {
        core::slice::from_raw_parts(data as *const u8, count as usize * fixhandle::HANDLE_SIZE)
    };
    let reply = operation("coordinate")
        .blob(name)
        .blob(bytes)
        .word(handles::address(destination) as u64)
        .call_with_current_continuation();
    let reply: Word<Runtime> = reply.try_into().expect("coordination returned no status");
    reply.read()
}

pub fn finish(value: Reference) -> ! {
    crate::locals::lift(&[value]);
    operation("return")
        .blob(&handles::bytes(value))
        .call_with_current_continuation();
    panic!("resumed a completed invocation")
}

fn attachments() -> Argument {
    use Argument::{Blob, Tuple, Value, Word};
    let (base, length) = crate::references::range();
    let mut entries = alloc::vec![Tuple(alloc::vec![
        Word(base as u64),
        Value(arca::Null::<Runtime>::new().into()),
        Word(crate::references::mapped() as u64),
        Word((length * fixhandle::HANDLE_SIZE) as u64),
        Word(1),
    ])];
    unsafe {
        for index in 0..crate::rt::MEMORY_IDX {
            if crate::rt::MEMORY_ATTACHED[index] {
                entries.push(Tuple(alloc::vec![
                    Word((index * SLOT_SIZE) as u64),
                    Blob(handles::bytes(crate::rt::MEMORY_HANDLES[index]).to_vec()),
                    Word((*crate::rt::MEMORIES[index]).size),
                    Word(0),
                    Word(0),
                ]));
            }
        }
        for index in 0..crate::rt::TABLE_IDX {
            if crate::rt::TABLES[index].is_null() {
                continue;
            }
            let table = &*crate::rt::TABLES[index];
            let attached = crate::rt::TABLE_ATTACHED[index];
            let backing = if attached {
                Blob(handles::bytes(crate::rt::TABLE_HANDLES[index]).to_vec())
            } else {
                Value(arca::Null::<Runtime>::new().into())
            };
            entries.push(Tuple(alloc::vec![
                Word(((crate::rt::NUM_MEMORIES + index) * SLOT_SIZE) as u64),
                backing,
                Word(table.memory_size as u64),
                Word(table.size as u64 * fixhandle::HANDLE_SIZE as u64),
                Word((!attached) as u64),
            ]));
        }
    }
    Tuple(entries)
}
