use crate::{Root, storage};
use fix::map::Map;
use fix::{Error, Tree, Value, ValueType};
use fixscheduler::kernel::{self, Execution};
pub use kernel::Advice;

pub trait Handler {
    fn advise(&self, advice: &Advice) -> Result<(), Error>;
    fn completed(&self, input: &Root, coupon: &Root) -> Result<bool, Error>;
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
            storage::identify(value)
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

pub fn resume<H: Handler>(
    mut execution: Execution,
    input: &Root,
    handler: &H,
) -> Result<Invocation, Error> {
    loop {
        match execution {
            Execution::Complete(value) => {
                let recommended = handler.completed(input, &value)?;
                return Ok(Invocation { value, recommended });
            }
            Execution::Advice {
                advice,
                continuation,
            } => {
                handler.advise(&advice)?;
                execution = continuation.resume()?;
            }
        }
    }
}
