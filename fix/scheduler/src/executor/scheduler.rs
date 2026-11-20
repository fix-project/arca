use crate::shell::{self, fields};
use crate::{kernel::Advice, shell::Value};
use alloc::{boxed::Box, vec::Vec};
use fixwasm::{Error, ThunkType, Value as AsValue, ValueType, backend::Tree};

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
    /// Obtain an Apply coupon for an evaluated combination.
    async fn apply_coupon(&self, input: &Value) -> Result<Value, Error> {
        crate::kernel::apply(input, &mut |advice| self.on_advice(advice)).await
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

    fn discovering(&self) -> bool {
        false
    }

    /// Evaluate without creating coupons, recursing through the scheduler's steps.
    /// Reconstructing Tags requires the scheduler to be a trusted tagger.
    async fn default_eval(&self, input: &Value) -> Result<Value, Error> {
        let evaluation = crate::progress::evaluation(1, input);
        let result = async {
            if input.value_type() == ValueType::Encode {
                let thunk = crate::storage::thunk(input)?;
                let forced = Box::pin(self.force(&thunk)).await?;
                let result = if input.is_strict()? {
                    self.load(&forced).await?
                } else {
                    shell::reference(&forced)?
                };
                Box::pin(self.eval(&result)).await
            } else if input.is_value()? {
                if input.value_type() == ValueType::Object {
                    self.load(input).await?;
                }
                Ok(input.clone())
            } else {
                let data = self.load(input).await?;
                let mut results = Vec::new();
                let children = fields(&data)?;
                for value in &children {
                    results.push(Box::pin(self.eval(value)).await?);
                }
                let result = Tree::new(&results);
                if data.is_tag()? {
                    shell::capture(result.inner().tag_unchecked()?)
                } else {
                    Ok(result.into_value())
                }
            }
        }
        .await;
        evaluation.finish(result)
    }

    /// Perform one thunk step without creating coupons.
    async fn default_think(&self, input: &Value) -> Result<Value, Error> {
        let evaluation = crate::progress::evaluation(4, input);
        let result = async {
            let kind = input.unsound_thunk_type()?;
            if kind == ThunkType::Identification {
                return crate::storage::reference(input);
            }
            let data = self.load(input).await?;
            let evaluated = Box::pin(self.eval(&data)).await?;
            match kind {
                ThunkType::Application => self.apply(&evaluated).await,
                ThunkType::Selection => self.select(&evaluated).await,
                _ => unreachable!(),
            }
        }
        .await;
        evaluation.finish(result)
    }

    /// Follow thunk results without creating coupons.
    async fn default_force(&self, input: &Value) -> Result<Value, Error> {
        let evaluation = crate::progress::evaluation(3, input);
        let result = async {
            let result = Box::pin(self.think(input)).await?;
            if matches!(result.value_type(), ValueType::Object | ValueType::Ref) {
                return Ok(result);
            }
            let next = crate::storage::thunk(&result)?;
            Box::pin(self.force(&next)).await
        }
        .await;
        evaluation.finish(result)
    }

    /// Run an ordinary application without requesting an Apply coupon.
    async fn default_apply(&self, input: &Value) -> Result<Value, Error> {
        let evaluation = crate::progress::evaluation(2, input);
        let result = crate::kernel::run(input, &mut |advice| self.on_advice(advice)).await;
        evaluation.finish(result)
    }

    /// Select a value without requesting a Select coupon.
    async fn default_select(&self, input: &Value) -> Result<Value, Error> {
        let evaluation = crate::progress::evaluation(6, input);
        let result = async {
            let target = fixwasm::backend::Tree::try_from(input.clone())?
                .get(0)?
                .ok_or(Error::OutOfBounds)?;
            if !matches!(target.value_type(), ValueType::Object | ValueType::Ref) {
                return Err(Error::WrongType);
            }
            self.load(&target).await?;
            crate::storage::selection(input)
        }
        .await;
        evaluation.finish(result)
    }

    /// Identify locally available backing data; report an error when it is unavailable.
    async fn default_load(&self, value: &Value) -> Result<Value, Error> {
        crate::storage::identify(value)?.ok_or(Error::OutOfBounds)
    }
}
