#![no_std]
extern crate alloc;

use effects::{storage, worker};
use fix::coupon::Statement;
use fix::{Blob, Error, Handle, Object, Tree, Value};
pub use fixscheduler::*;
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

pub fn run<E: Executor>(
    combination: &Handle<Object<Tree>, fix::Focus>,
    graph: &Graph,
    executor: &E,
) -> Result<Root, Error> {
    assert_eq!(combination.len()?, 4);
    let mode = Mode::read(&root(combination.get(0))?)?;
    let worker = worker::id()?;
    let execute = match mode {
        Mode::Eval => Statement::Eval,
        Mode::Think => Statement::Think,
        Mode::Todo => {
            if worker != 0 {
                loop {
                    let notification = worker::listen()?;
                    if worker::stopped()? {
                        break;
                    }
                    notification.wait()?;
                }
                return Ok(root(Tree::from_iter(core::iter::empty::<&Root>()))?);
            }
            let result = todo::run(&root(combination.get(3))?)?;
            worker::finish(&result)?;
            return Ok(result);
        }
    };

    let input = if worker == 0 {
        Some(Request::new(execute, &root(combination.get(3))?)?)
    } else {
        None
    };
    let result = graph.run(executor, input)?;
    finish(result)
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
