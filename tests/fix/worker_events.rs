#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use fix::{Blob, Error, Handle, Object, Tree, Value, tree};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

fn request(name: &[u8], arguments: &[u64]) -> Result<(u64, u64), Error> {
    let mut fields = Vec::with_capacity(arguments.len() + 1);
    fields.push(Blob::create(name).capture()?);
    for argument in arguments {
        fields.push(argument.capture()?);
    }
    let reply = Handle::<Object<Tree>, _>::try_from(Tree::from_iter(&fields).effect()?)?;
    Ok((reply.get(0).read_u64()?, reply.get(1).read_u64()?))
}

fn call(name: &[u8], arguments: &[u64]) -> Result<u64, Error> {
    let (status, value) = request(name, arguments)?;
    assert_eq!(status, 0);
    Ok(value)
}

fn pop(queue: u64) -> Result<u64, Error> {
    loop {
        let (status, value) = request(b"queue_pop", &[queue])?;
        if status == 0 {
            return Ok(value);
        }
        assert_eq!(status, 1);
        call(b"worker_yield", &[])?;
    }
}

fn wake_one_queue(worker: u64, workers: u64) -> Result<u64, Error> {
    if worker == 0 {
        assert_eq!(call(b"queue_create", &[workers])?, 0);
        assert_eq!(call(b"queue_create", &[workers])?, 1);
        let mut seen = alloc::vec![false; (32 * (workers - 1)) as usize];
        let mut sum = 0;
        for round in 0..32 {
            for index in 1..workers {
                call(b"queue_push", &[0, round * (workers - 1) + index])?;
                call(b"worker_wake_one", &[])?;
            }
            for _ in 1..workers {
                let value = pop(1)?;
                let entry = &mut seen[(value - 1) as usize];
                assert!(!*entry);
                *entry = true;
                sum += value;
            }
        }
        return Ok(sum);
    }
    while call(b"queue_count", &[])? < 2 {
        call(b"worker_yield", &[])?;
    }
    loop {
        let epoch = call(b"worker_listen", &[])?;
        let (status, value) = request(b"queue_pop", &[0])?;
        if status == 0 {
            call(b"queue_push", &[1, value])?;
        } else {
            assert_eq!(status, 1);
            call(b"worker_wait", &[epoch])?;
        }
    }
}

#[fix::apply]
fn apply<'a>(combination: &'a Handle<Object<Tree>, fix::Focus>) -> Result<impl Value + 'a, Error> {
    let mode = combination.get(2).read_u64()?;
    if mode == 0 {
        for _ in 0..64 {
            let epoch = call(b"worker_listen", &[])?;
            call(b"worker_wake", &[])?;
            call(b"worker_wait", &[epoch])?;
            assert_ne!(call(b"worker_listen", &[])?, epoch);
        }
        return Ok(tree![64u64]);
    }
    let worker = call(b"worker_config", &[0])?;
    let workers = call(b"worker_config", &[1])?;
    if mode == 3 {
        return Ok(tree![wake_one_queue(worker, workers)?]);
    }
    if worker == 0 {
        assert_eq!(call(b"queue_create", &[workers])?, 0);
        assert_eq!(call(b"queue_create", &[workers])?, 1);
        for _ in 0..32 {
            for _ in 1..workers {
                pop(0)?;
            }
            call(b"worker_wake", &[])?;
            for _ in 1..workers {
                pop(1)?;
            }
        }
        for _ in 1..workers {
            pop(0)?;
        }
        for _ in 0..workers * 2 {
            call(b"worker_yield", &[])?;
        }
        if mode == 2 {
            Blob::create(b"malformed effect").effect()?;
            unreachable!();
        }
        Ok(tree![32 * (workers - 1)])
    } else {
        while call(b"queue_count", &[])? < 2 {
            call(b"worker_yield", &[])?;
        }
        for _ in 0..32 {
            let epoch = call(b"worker_listen", &[])?;
            call(b"queue_push", &[0, worker])?;
            call(b"worker_wait", &[epoch])?;
            call(b"queue_push", &[1, worker])?;
        }
        let epoch = call(b"worker_listen", &[])?;
        call(b"queue_push", &[0, worker])?;
        call(b"worker_wait", &[epoch])?;
        unreachable!()
    }
}
