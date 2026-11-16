use crate::shell::{self, as_tree, capture, fields};
use crate::{kernel::Advice, shell::Value};
use alloc::{boxed::Box, vec::Vec};
use fixwasm::{Any, Error, Handle, Object, ThunkType, Value as AsValue, ValueType, backend::Tree};

/// Fix evaluation with overridable steps. Defaults recurse through the trait methods.
#[allow(async_fn_in_trait)]
pub trait Scheduler {
    /// Evaluate a value, awaiting required dependencies.
    async fn eval(&self, input: &Value) -> Result<Value, Error> {
        self.default_eval(input).await
    }
    /// Perform one thunk step.
    async fn think(&self, input: &Value) -> Result<Value, Error> {
        self.default_think(input).await
    }
    /// Follow thunk results to an Object or Ref.
    async fn force(&self, input: &Value) -> Result<Value, Error> {
        self.default_force(input).await
    }
    /// Apply an evaluated combination.
    async fn apply(&self, input: &Value) -> Result<Value, Error> {
        self.default_apply(input).await
    }
    /// Select from an evaluated combination.
    async fn select(&self, input: &Value) -> Result<Value, Error> {
        self.default_select(input).await
    }
    /// Obtain backing data, awaiting acquisition.
    async fn load(&self, value: &Value) -> Result<Value, Error> {
        self.default_load(value).await
    }
    /// Observe advice synchronously without replying to its emitting invocation.
    fn on_advice(&self, _advice: &Advice) -> Result<(), Error> {
        Ok(())
    }

    /// Evaluate without creating coupons, recursing through the scheduler's steps.
    async fn default_eval(&self, input: &Value) -> Result<Value, Error> {
        if input.value_type() == ValueType::Encode {
            let thunk = thunk(self, input).await?;
            let forced = Box::pin(self.force(&thunk)).await?;
            let result = if input.is_strict()? {
                self.load(&forced).await?
            } else {
                shell::reference(&forced)?
            };
            Box::pin(self.eval(&result)).await
        } else if input.is_value()? {
            Ok(input.clone())
        } else {
            let data = self.load(input).await?;
            let mut results = Vec::new();
            for value in fields(&data)? {
                results.push(Box::pin(self.eval(&value)).await?);
            }
            Ok(Tree::new(&results).into_value())
        }
    }

    /// Perform one thunk step without creating coupons.
    async fn default_think(&self, input: &Value) -> Result<Value, Error> {
        let data = self.load(input).await?;
        let kind = input.unsound_thunk_type()?;
        if kind == ThunkType::Identification {
            return shell::reference(&data);
        }
        let evaluated = Box::pin(self.eval(&data)).await?;
        match kind {
            ThunkType::Application => self.apply(&evaluated).await,
            ThunkType::Selection => self.select(&evaluated).await,
            _ => unreachable!(),
        }
    }

    /// Follow thunk results without creating coupons.
    async fn default_force(&self, input: &Value) -> Result<Value, Error> {
        let result = Box::pin(self.think(input)).await?;
        if matches!(result.value_type(), ValueType::Object | ValueType::Ref) {
            return Ok(result);
        }
        let next = thunk(self, &result).await?;
        Box::pin(self.force(&next)).await
    }

    /// Run an ordinary application without requesting an Apply coupon.
    async fn default_apply(&self, input: &Value) -> Result<Value, Error> {
        crate::kernel::run(input, &mut |advice| self.on_advice(advice)).await
    }

    /// Select a value without requesting a Select coupon.
    async fn default_select(&self, input: &Value) -> Result<Value, Error> {
        crate::storage::selection(input)
    }

    /// Identify locally available backing data; report an error when it is unavailable.
    async fn default_load(&self, value: &Value) -> Result<Value, Error> {
        crate::storage::identify(value)?.ok_or(Error::OutOfBounds)
    }
}

pub(crate) async fn thunk(
    policy: &(impl Scheduler + ?Sized),
    value: &Value,
) -> Result<Value, Error> {
    let data = policy.load(value).await?;
    Ok(match value.unsound_thunk_type()? {
        ThunkType::Application => capture(as_tree(&data)?.application())?,
        ThunkType::Identification => {
            capture(Handle::<Object<Any>>::try_from(data.inner())?.identification())?
        }
        ThunkType::Selection => capture(as_tree(&data)?.selection())?,
    })
}
