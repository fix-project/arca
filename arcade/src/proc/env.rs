use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};
use kernel::prelude::RwLock;

#[derive(Clone, Default)]
pub struct Env {
    env: Arc<RwLock<BTreeMap<String, String>>>,
}
