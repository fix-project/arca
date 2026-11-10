use crate::{Any, Error, Handle, Object, Tree, Value, tree};

fn request(name: &[u8], arguments: Handle<Object<Tree>>) -> Result<(u64, Handle<Any>), Error> {
    arguments.coordinate(name)
}

pub struct Entry {
    pub version: u64,
    pub value: Handle<Any>,
}

/// A shared map scoped to the current scheduler run. Keys use their exact
/// 32-byte handle representations; capture a constructed key to reuse it.
#[derive(Clone, Copy)]
pub struct Map(u64);

impl Map {
    pub fn id(self) -> u64 {
        self.0
    }

    pub fn open(index: u64) -> Self {
        Self(index)
    }

    pub fn create() -> Result<Self, Error> {
        let (status, value) = request(b"map_create", tree![])?;
        assert_eq!(status, 0, "map create failed");
        Ok(Self(value.read_u64()?))
    }

    pub fn count() -> Result<u64, Error> {
        let (status, value) = request(b"map_count", tree![])?;
        assert_eq!(status, 0, "map count failed");
        value.read_u64()
    }

    pub fn get(&self, key: impl Value) -> Result<Option<Entry>, Error> {
        let (status, value) = request(b"map_get", tree![self.0, key])?;
        match status {
            0 => {
                let entry = Handle::<crate::Object<crate::Tree>>::try_from(&value)?;
                Ok(Some(Entry {
                    version: entry.get(0).read_u64()?,
                    value: entry.get(1).capture()?,
                }))
            }
            1 => Ok(None),
            _ => panic!("map get failed"),
        }
    }

    pub fn insert(&self, key: impl Value, value: impl Value) -> Result<Option<Handle<Any>>, Error> {
        let key = key.capture()?;
        loop {
            let entry = self.get(&key)?;
            let version = entry.as_ref().map_or(0, |entry| entry.version);
            if self.compare_exchange(&key, version, &value)? {
                return Ok(entry.map(|entry| entry.value));
            }
        }
    }

    /// Version zero inserts an absent key. Versions are never reused within a run.
    pub fn compare_exchange(
        &self,
        key: impl Value,
        version: u64,
        value: impl Value,
    ) -> Result<bool, Error> {
        let (status, _) = request(b"map_compare_exchange", tree![self.0, key, version, value])?;
        match status {
            0 => Ok(true),
            1 => Ok(false),
            _ => panic!("map compare-exchange failed"),
        }
    }

    pub fn remove(&self, key: impl Value) -> Result<Option<Handle<Any>>, Error> {
        let (status, value) = request(b"map_remove", tree![self.0, key])?;
        match status {
            0 => Ok(Some(value)),
            1 => Ok(None),
            _ => panic!("map remove failed"),
        }
    }
}
