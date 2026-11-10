#![no_std]
extern crate alloc;

use fixwasm::{Blob, Error, Handle, Object, Tree, Value, map::Map, tree};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

fn call(name: &[u8], arguments: impl Value) -> Result<u64, Error> {
    let arguments = Handle::<Object<Tree>>::try_from(arguments.capture()?)?;
    let mut fields = alloc::vec![Blob::create(name).capture()?];
    for i in 0..usize::try_from(arguments.len()?)? {
        fields.push(arguments.get(i).capture()?);
    }
    let reply = Handle::<Object<Tree>>::try_from(Tree::from_iter(&fields).effect()?)?;
    assert_eq!(reply.get(0).read_u64()?, 0);
    reply.get(1).read_u64()
}

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    let worker = call(b"worker_config", tree![0u64])?;
    let workers = call(b"worker_config", tree![1u64])?;
    if worker == 0 {
        Map::create()?;
        call(b"queue_create", tree![workers])?;
        call(b"worker_wake", tree![])?;
    } else {
        loop {
            let epoch = call(b"worker_listen", tree![])?;
            if Map::count()? != 0 && call(b"queue_count", tree![])? != 0 {
                break;
            }
            call(b"worker_wait", tree![epoch])?;
        }
    }
    let map = Map::open(0);
    let thunk = tree![worker, 0u64].application().capture()?;
    assert!(map.compare_exchange(&thunk, 0, 11u64)?);
    assert_eq!(map.get(&thunk)?.unwrap().value.read_u64()?, 11);
    map.remove(&thunk)?;
    let counter = combination.get(3).capture()?;
    for _ in 0..64 {
        loop {
            let entry = map.get(&counter)?;
            let version = entry.as_ref().map_or(0, |entry| entry.version);
            let value = match entry {
                Some(entry) => entry.value.read_u64()?,
                None => 0,
            };
            if map.compare_exchange(&counter, version, value + 1)? {
                break;
            }
        }
    }
    if worker != 0 {
        call(b"queue_push", tree![0u64, worker])?;
        call(b"worker_wake", tree![])?;
        loop {
            let epoch = call(b"worker_listen", tree![])?;
            call(b"worker_wait", tree![epoch])?;
        }
    }
    for _ in 1..workers {
        loop {
            let epoch = call(b"worker_listen", tree![])?;
            let reply = Handle::<Object<Tree>>::try_from(
                tree![Blob::create(b"queue_pop"), 0u64].effect()?,
            )?;
            match reply.get(0).read_u64()? {
                0 => break,
                1 => {
                    call(b"worker_wait", tree![epoch])?;
                }
                _ => panic!("queue pop failed"),
            }
        }
    }
    let entry = map.get(&counter)?.unwrap();
    assert_eq!(entry.value.read_u64()?, workers * 64);
    assert!(!map.compare_exchange(&counter, 0, 999u64)?);
    assert_eq!(map.remove(&counter)?.unwrap().read_u64()?, workers * 64);
    assert!(map.get(&counter)?.is_none());
    assert!(map.compare_exchange(&counter, 0, 7u64)?);
    assert!(!map.compare_exchange(&counter, entry.version, 999u64)?);
    assert_ne!(map.get(&counter)?.unwrap().version, entry.version);
    map.remove(&counter)?;

    let other = Map::create()?;
    assert_ne!(other.id(), map.id());
    assert!(other.insert(5u64, 8u64)?.is_none());
    assert_eq!(other.insert(5u64, 9u64)?.unwrap().read_u64()?, 8);
    assert_eq!(
        Map::open(other.id()).get(5u64)?.unwrap().value.read_u64()?,
        9
    );
    assert!(map.get(5u64)?.is_none());

    let keys = Handle::<Object<Tree>>::try_from(combination.get(2))?;
    for index in 0..usize::try_from(keys.len()?)? {
        let pair = Handle::<Object<Tree>>::try_from(keys.get(index))?;
        assert!(map.compare_exchange(
            pair.get(0),
            0,
            tree![index as u64, &keys, tree![index as u64].application()]
        )?);
        if pair.get(0).raw_handle_bytes()? != pair.get(1).raw_handle_bytes()? {
            assert!(map.get(pair.get(1))?.is_none());
        }
        let entry = map.get(pair.get(0))?.unwrap();
        let value = Handle::<Object<Tree>>::try_from(&entry.value)?;
        assert_eq!(value.get(0).read_u64()?, index as u64);
        let restored = Handle::<Object<Tree>>::try_from(value.get(1))?;
        assert!(restored.get(index).equals(&pair)?);
        assert_eq!(value.get(2).value_type()?, fixwasm::ValueType::Thunk);
    }
    for index in (0..usize::try_from(keys.len()?)?).rev() {
        let pair = Handle::<Object<Tree>>::try_from(keys.get(index))?;
        assert!(map.remove(pair.get(0))?.is_some());
        assert!(map.get(pair.get(0))?.is_none());
    }
    Ok(tree![workers * 64])
}
