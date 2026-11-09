use super::{Root, as_tree, root};
use fix::coupon::Statement;
use fix::{Any, Blob, Error, Handle, Object, ThunkType, Tree, Value, ValueType, tree};

fn request(value: impl Value) -> Result<(u64, Root), Error> {
    let reply = Handle::<Object<Tree>, _>::try_from(value.effect()?)?;
    Ok((reply.get(0).read_u64()?, root(reply.get(1))?))
}
pub fn coordinate(name: &[u8], arguments: &[Root]) -> Result<(u64, Root), Error> {
    let (status, value) = Tree::from_iter(arguments).coordinate(name)?;
    Ok((status, alloc::rc::Rc::new(value)))
}
fn coordinated(name: &[u8], arguments: &[Root]) -> Result<Root, Error> {
    let (status, value) = coordinate(name, arguments)?;
    assert_eq!(status, 0, "scheduler coordination failed");
    Ok(value)
}
fn effect(name: &[u8], arguments: &[Root]) -> Result<Root, Error> {
    let name = root(Blob::create(name))?;
    let values = core::iter::once(&name).chain(arguments);
    let (status, value) = request(Tree::from_iter(values))?;
    assert_eq!(status, 0, "scheduler effect failed");
    Ok(value)
}

pub mod storage {
    use super::*;

    pub fn cache(value: &Root, completed: bool) -> Result<Root, Error> {
        coordinated(
            b"storage_cache",
            &[value.clone(), root(u64::from(completed))?],
        )
    }
    pub fn recommend(value: &Root) -> Result<Root, Error> {
        coordinated(b"storage_cache", &[value.clone(), root(2u64)?])
    }
    pub fn put(value: &Root) -> Result<Root, Error> {
        effect(b"storage_put", core::slice::from_ref(value))
    }
    pub fn get_todo(value: &Root) -> Result<Option<Root>, Error> {
        let (status, value) = request(tree![Blob::create(b"storage_get_todo"), value])?;
        match status {
            0 => Ok(Some(value)),
            1 => Ok(None),
            _ => panic!("todo metadata read failed"),
        }
    }
    pub fn set_todo(value: &Root, metadata: &Root) -> Result<(), Error> {
        effect(b"storage_set_todo", &[value.clone(), metadata.clone()])?;
        Ok(())
    }
    pub fn object(value: &Root) -> Result<Root, Error> {
        let (status, result) = coordinate(b"storage_identify", core::slice::from_ref(value))?;
        match status {
            0 => Ok(result),
            1 => super::super::todo::acquire(value)?.ok_or(Error::OutOfBounds),
            _ => panic!("object identification failed"),
        }
    }
    pub fn reference(value: &Root) -> Result<Root, Error> {
        if value.value_type()? == ValueType::Ref {
            return Ok(value.clone());
        }
        let data = Handle::<Object<Any>, _>::try_from(value.as_ref())?;
        root(data.reference())
    }
    pub fn thunk(value: &Root) -> Result<Root, Error> {
        let data = object(value)?;
        match value.unsound_thunk_type()? {
            ThunkType::Application => root(as_tree(&data)?.application()),
            ThunkType::Identification => {
                root(Handle::<Object<Any>, _>::try_from(data.as_ref())?.identification())
            }
            ThunkType::Selection => root(as_tree(&data)?.selection()),
        }
    }

    pub fn claims(statement: Statement, input: &Root) -> Result<Root, Error> {
        coordinated(b"storage_claims", &[root(statement as u64)?, input.clone()])
    }

    pub fn select(input: &Root) -> Result<Root, Error> {
        effect(b"storage_select", core::slice::from_ref(input))
    }

    pub fn eval(input: &Root) -> Result<Root, Error> {
        effect(b"storage_eval", core::slice::from_ref(input))
    }
}

pub mod callable {
    use super::*;

    pub fn apply<H: crate::advice::Handler>(
        combination: &Root,
        handler: &H,
    ) -> Result<crate::advice::Invocation, Error> {
        crate::advice::resume(
            request(tree![Blob::create(b"callable_apply"), combination])?,
            combination,
            handler,
        )
    }

    pub fn invoke<H: crate::advice::Handler>(
        combination: &Root,
        handler: &H,
    ) -> Result<crate::advice::Invocation, Error> {
        crate::advice::resume(
            coordinate(b"callable_invoke", core::slice::from_ref(combination))?,
            combination,
            handler,
        )
    }
}

pub mod worker {
    use super::*;

    pub struct Notification(u64);

    pub fn listen() -> Result<Notification, Error> {
        Ok(Notification(
            coordinated(b"worker_listen", &[])?.read_u64()?,
        ))
    }

    impl Notification {
        pub fn wait(self) -> Result<(), Error> {
            coordinated(b"worker_wait", &[root(self.0)?])?;
            Ok(())
        }
    }

    pub fn wake() -> Result<(), Error> {
        coordinated(b"worker_wake", &[])?;
        Ok(())
    }

    pub(super) fn wake_one() -> Result<(), Error> {
        coordinated(b"worker_wake_one", &[])?;
        Ok(())
    }

    fn config(field: u64) -> Result<u64, Error> {
        coordinated(b"worker_config", &[root(field)?])?.read_u64()
    }

    pub fn id() -> Result<u64, Error> {
        config(0)
    }

    pub fn count() -> Result<u64, Error> {
        config(1)
    }

    pub fn returns_coupon() -> Result<bool, Error> {
        Ok(config(3)? != 0)
    }

    pub fn stopped() -> Result<bool, Error> {
        Ok(config(2)? != 0)
    }

    pub fn finish(value: &Root) -> Result<(), Error> {
        coordinated(b"worker_finish", core::slice::from_ref(value))?;
        Ok(())
    }
}

pub mod queue {
    use super::*;

    pub fn count() -> Result<u64, Error> {
        coordinated(b"queue_count", &[])?.read_u64()
    }

    pub struct Queue(u64);

    impl Queue {
        pub fn open(index: u64) -> Self {
            Self(index)
        }

        pub fn create(capacity: u64) -> Result<Self, Error> {
            let queue = Self(coordinated(b"queue_create", &[root(capacity)?])?.read_u64()?);
            worker::wake()?;
            Ok(queue)
        }

        pub fn pop(&self) -> Result<Option<Root>, Error> {
            let (status, value) = tree![self.0].coordinate(b"queue_pop")?;
            match status {
                0 => Ok(Some(alloc::rc::Rc::new(value))),
                1 => Ok(None),
                _ => panic!("queue pop failed"),
            }
        }

        pub fn push(&self, value: &Root) -> Result<bool, Error> {
            let (status, _) = tree![self.0, value].coordinate(b"queue_push")?;
            match status {
                0 => {
                    worker::wake_one()?;
                    Ok(true)
                }
                2 => Ok(false),
                _ => panic!("queue push failed"),
            }
        }
    }
}
