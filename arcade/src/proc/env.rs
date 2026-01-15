use alloc::{collections::btree_map::BTreeMap, string::String, string::ToString, sync::Arc};
use common::util::rwlock::RwLock;

#[derive(Clone, Default)]
pub struct Env {
    env: Arc<RwLock<BTreeMap<String, String>>>,
}

impl Env {
    pub fn set(&self, key: &str, value: &str) {
        let mut env = self.env.write();
        env.insert(key.to_string(), value.to_string());
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let env = self.env.read();
        env.get(key).cloned()
    }
}
