#![no_std]
extern crate alloc;

use fix::coupon::Statement;
use fix::{Blob, Error, Handle, Object, Tree, Value};
pub use fixscheduler::{kernel, shell, storage};
use kernel::worker;
pub use shell::{Root, as_tree, child, fields, root};
pub mod advice;
pub mod collector;
pub mod evaluation;
pub mod request;
pub mod serial;
pub mod todo;
pub use collector::{CachedClaims, Claims, Collector, Evidence, InputRetention, Proofs};
pub use evaluation::{EvaluationState, EvaluationStep};
pub use request::Request;
pub type Key = [u8; 32];
pub type Resolved = alloc::collections::BTreeMap<(u64, Key), Evidence>;
pub mod composition;
pub mod graph;
pub use composition::*;
pub use graph::{Executor, Graph};

enum Mode {
    Eval,
    Think,
    Todo,
}

impl Mode {
    fn read(value: &Root) -> Result<Self, Error> {
        let value = Handle::<Object<Blob>, _>::try_from(value.as_ref())?;
        let mut name = alloc::vec![0; usize::try_from(value.len()?)?];
        value.read_into(0, &mut name)?;
        match name.as_slice() {
            b"eval" => Ok(Self::Eval),
            b"think" => Ok(Self::Think),
            b"todo" => Ok(Self::Todo),
            _ => Err(Error::WrongType),
        }
    }
}

pub fn input(combination: &Handle<Object<Tree>, fix::Focus>) -> Result<Option<Request>, Error> {
    assert_eq!(combination.len()?, 4);
    let mode = Mode::read(&root(combination.get(0))?)?;
    let statement = match mode {
        Mode::Eval => Statement::Eval,
        Mode::Think => Statement::Think,
        Mode::Todo => {
            if worker::id()? == 0 {
                let result = todo::run(&root(combination.get(3))?)?;
                worker::finish(&result)?;
            }
            return Ok(None);
        }
    };
    Ok(Some(Request::new(statement, &root(combination.get(3))?)?))
}

pub fn wait() -> Result<Root, Error> {
    loop {
        let notification = worker::listen()?;
        if worker::stopped()? {
            return root(fix::tree![]);
        }
        notification.wait()?;
    }
}

pub fn finish(result: Evidence) -> Result<Root, Error> {
    let result = if worker::returns_coupon()? {
        let coupon = child(&result.witness, 0)?;
        storage::cache(&coupon, true)?;
        coupon
    } else {
        result.result
    };
    worker::finish(&result)?;
    Ok(result)
}

pub fn shared_map(index: u64) -> Result<fix::map::Map, Error> {
    if worker::id()? == 0 {
        let map = fix::map::Map::create()?;
        assert_eq!(map.id(), index);
        worker::wake()?;
        Ok(map)
    } else {
        loop {
            let notification = worker::listen()?;
            if fix::map::Map::count()? > index {
                return Ok(fix::map::Map::open(index));
            }
            notification.wait()?;
        }
    }
}
