#![no_std]

use fix::{Error, Handle, Object, Tree, Value};
use scheduler_example::{
    CachedClaims, Collector, Effects, InputRetention, Services, TodoAcquisition,
    advice::Canonicalize,
    kernel::{map::Map, worker},
    root,
};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[fix::apply]
fn apply(combination: &Handle<Object<Tree>, fix::Focus>) -> Result<impl Value, Error> {
    if worker::id()? != 0 {
        return scheduler_example::wait();
    }
    let Some(input) = scheduler_example::input(combination)? else {
        return scheduler_example::wait();
    };
    let advice = Canonicalize::new(Map::create()?);
    let backend = Services {
        proofs: Collector::new(
            root(combination.get(2))?,
            advice,
            InputRetention::new(Map::create()?),
        ),
        claims: CachedClaims::default(),
        operations: Effects {
            acquisition: TodoAcquisition,
            advice,
        },
    };
    scheduler_example::finish(scheduler_example::serial::evaluate(&backend, &input)?)
}
