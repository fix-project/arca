#![no_std]
extern crate alloc;

#[path = "parallel/evaluation.rs"]
mod evaluation;
#[path = "parallel/graph.rs"]
mod graph;
#[path = "parallel/policy.rs"]
mod policy;
#[path = "parallel/request.rs"]
mod request;
use fixscheduler::{
    kernel::worker,
    shell::{Root, as_tree, child, fields, root},
};
use fixwasm::{Error, Handle, Object, Tree, Value, coupon::Statement, map::Map};
use graph::Graph;
use policy::*;
use request::Request;
type Resolved = alloc::collections::BTreeMap<(u64, [u8; 32]), Evidence>;

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    let graph = Graph::join()?;
    let scheduler = Scheduler::new(root(combination.get(2))?, shared_map(5)?, shared_map(6)?);
    assert_eq!(combination.len()?, 4);
    let mode = combination.get(0);
    let input = root(combination.get(3))?;
    if mode.equals(fixwasm::Blob::create(b"todo"))? {
        if worker::id()? == 0 {
            let result = todo(&input)?;
            worker::finish(&result)?;
            return Ok(result);
        }
        return wait();
    }
    let statement = if mode.equals(fixwasm::Blob::create(b"eval"))? {
        Statement::Eval
    } else if mode.equals(fixwasm::Blob::create(b"think"))? {
        Statement::Think
    } else {
        return Err(Error::WrongType);
    };
    let input = if worker::id()? == 0 {
        Some(Request::new(statement, &input)?)
    } else {
        None
    };
    finish(graph.run(&scheduler, input)?)
}

fn shared_map(index: u64) -> Result<Map, Error> {
    if worker::id()? == 0 {
        let map = Map::create()?;
        assert_eq!(map.id(), index);
        worker::wake()?;
        Ok(map)
    } else {
        loop {
            let notification = worker::listen()?;
            if Map::count()? > index {
                return Ok(Map::open(index));
            }
            notification.wait()?;
        }
    }
}
