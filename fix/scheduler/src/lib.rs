#![no_std]
extern crate alloc;

use alloc::{collections::BTreeMap, rc::Rc, vec::Vec};
pub mod advice;
pub mod collector;
pub mod evaluation;
pub use collector::{
    CachedClaims, Claims, Collector, DirectRetention, Evidence, InputRetention, Proofs, Retention,
};
pub mod effects;
pub mod todo;
pub use evaluation::{EvaluationState, EvaluationStep};
use fix::{Any, Capture, Error, Handle, Object, Table, Tree, Value};
mod request;
pub use request::Request;

pub type Root = Rc<Handle<Any, Capture>>;
pub type Key = [u8; 32];
pub type Resolved = BTreeMap<(u64, Key), Evidence>;

pub fn root(value: impl Value) -> Result<Root, Error> {
    Ok(Rc::new(value.capture()?))
}
pub fn as_tree(value: &Root) -> Result<Handle<Object<Tree>, &Capture>, Error> {
    Handle::<Object<Tree>, _>::try_from(value.as_ref())
}
pub fn child(value: &Root, index: usize) -> Result<Root, Error> {
    root(as_tree(value)?.get(index))
}

pub fn fields(value: &Root) -> Result<Vec<Root>, Error> {
    let tree = as_tree(value)?;
    let mut table = unsafe { Table::from_index(3) };
    let attached = table.attach(&tree)?;
    attached.iter().map(root).collect()
}
