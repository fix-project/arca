#![no_std]

use fix::{Error, Handle, Object, Tree, Value};
use scheduler_example::{
    CachedClaims, Collector, Effects, Execution, Graph, InputRetention, Services, TodoAcquisition,
    advice::Canonicalize, root,
};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[fix::apply]
fn apply(combination: &Handle<Object<Tree>, fix::Focus>) -> Result<impl Value, Error> {
    let graph = Graph::join()?;
    let advice = Canonicalize::new(scheduler_example::shared_map(5)?);
    let retention = InputRetention::new(scheduler_example::shared_map(6)?);
    let executor = Execution {
        backend: Services {
            proofs: Collector::new(root(combination.get(2))?, advice, retention),
            claims: CachedClaims::default(),
            operations: Effects {
                acquisition: TodoAcquisition,
                advice,
            },
        },
    };
    scheduler_example::run(combination, &graph, &executor)
}
