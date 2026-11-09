use crate::effects;
use crate::{Root, as_tree, child};
use fix::map::Map;
use fix::{Blob, Error, Tree, Value, ValueType};

pub struct Advice {
    pub target: Root,
    pub key: Root,
    pub payload: Root,
}

impl Advice {
    pub fn key_is(&self, key: &str) -> Result<bool, Error> {
        self.key.equals(Blob::create(key.as_bytes()))
    }
}

pub trait Handler {
    fn advise(&self, advice: &Advice) -> Result<(), Error>;
    fn completed(&self, input: &Root, coupon: &Root) -> Result<bool, Error>;
}

#[derive(Clone, Copy)]
pub struct Ignore;
impl Handler for Ignore {
    fn advise(&self, _: &Advice) -> Result<(), Error> {
        Ok(())
    }
    fn completed(&self, _: &Root, _: &Root) -> Result<bool, Error> {
        Ok(false)
    }
}

#[derive(Clone, Copy)]
pub struct Canonicalize {
    pending: Map,
}
impl Canonicalize {
    pub fn new(pending: Map) -> Self {
        Self { pending }
    }

    fn target(value: &Root) -> Result<Option<Root>, Error> {
        if matches!(value.value_type()?, ValueType::Thunk | ValueType::Ref) {
            match effects::coordinate(b"storage_identify", core::slice::from_ref(value))? {
                (0, target) => Ok(Some(target)),
                (1, _) => Ok(None),
                _ => Err(Error::WrongType),
            }
        } else {
            Ok(Some(value.clone()))
        }
    }
}
impl Handler for Canonicalize {
    fn advise(&self, advice: &Advice) -> Result<(), Error> {
        if advice.key_is("fix::canonicalize")?
            && let Some(target) = Self::target(&advice.target)?
        {
            self.pending
                .compare_exchange(&target, 0, Tree::create(&[] as &[Root]))?;
        }
        Ok(())
    }
    fn completed(&self, input: &Root, _: &Root) -> Result<bool, Error> {
        Ok(self.pending.remove(input)?.is_some())
    }
}

pub struct Invocation {
    pub value: Root,
    pub recommended: bool,
}

pub(crate) fn resume<H: Handler>(
    mut reply: (u64, Root),
    input: &Root,
    handler: &H,
) -> Result<Invocation, Error> {
    loop {
        match reply {
            (0, result) => {
                let recommended = handler.completed(input, &result)?;
                return Ok(Invocation {
                    value: result,
                    recommended,
                });
            }
            (2, event) => {
                if as_tree(&event)?.len()? != 4 {
                    return Err(Error::WrongType);
                }
                let token = child(&event, 0)?;
                handler.advise(&Advice {
                    target: child(&event, 1)?,
                    key: child(&event, 2)?,
                    payload: child(&event, 3)?,
                })?;
                reply = effects::coordinate(b"callable_resume", &[token])?;
            }
            _ => return Err(Error::WrongType),
        }
    }
}
