use crate::executor::Scheduler;
use crate::kernel;
use crate::shell::{self, Value, as_tree, capture, child, fields};
use crate::storage;
use alloc::{boxed::Box, vec::Vec};
pub use fixwasm::coupon::Exchange;
use fixwasm::{Error, ThunkType, Value as AsValue, ValueType, backend::Tree, tree};

/// Exchange explicitly supplied coupons for a requested conclusion.
pub async fn exchange(
    rule: Exchange,
    lhs: &Value,
    rhs: &Value,
    coupons: &[Tree],
    on_advice: &mut dyn FnMut(&kernel::Advice) -> Result<(), Error>,
) -> Result<Value, Error> {
    kernel::invoke(&request(rule, lhs, rhs, coupons)?, on_advice).await
}

/// Build an exchange request without invoking the collector.
pub fn request(rule: Exchange, lhs: &Value, rhs: &Value, coupons: &[Tree]) -> Result<Value, Error> {
    let witnesses = coupons
        .iter()
        .map(|coupon| {
            let claim = crate::storage::identify(&crate::shell::child(coupon, 1)?)?
                .ok_or(Error::OutOfBounds)?;
            capture(tree![coupon, claim])
        })
        .collect::<Result<Vec<_>, Error>>()?;
    capture(tree![
        0u64,
        &callable()?,
        rule as u64,
        lhs,
        rhs,
        fixwasm::Tree::from_iter(&witnesses)
    ])
}
/// Return the collector configured for the current scheduler invocation.
pub fn callable() -> Result<Value, Error> {
    fixwasm::backend::combination()
        .get(3)?
        .ok_or(Error::OutOfBounds)
}
/// Find available coupons authored by this collector or the kernel.
pub fn claims(code: u64, input: &Value) -> Result<Vec<Value>, Error> {
    let mut results = Vec::new();
    for coupon in fields(&storage::claims(code, input)?)? {
        if !coupon.is_tag()? || as_tree(&coupon)?.len()? != 2 {
            continue;
        }
        let author = child(&coupon, 0)?;
        if !(author.is_comparable() && author.equals(&callable()?)?) {
            let Ok(kernel) = as_tree(&author) else {
                continue;
            };
            if kernel.is_tag()? || kernel.len()? != 0 {
                continue;
            }
        }
        let Some(claim) = storage::identify(&child(&coupon, 1)?)? else {
            continue;
        };
        if as_tree(&claim)?.len()? != 3 {
            continue;
        }
        results.push(coupon);
    }
    Ok(results)
}
fn produced(
    coupon: Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let coupon = self::coupon(coupon)?;
    retain(&coupon)?;
    Ok(coupon.into_value())
}
/// Validate a coupon against this collector, the operation, and its input.
pub fn validate(coupon: Value, operation: u64, input: &Value) -> Result<Option<Value>, Error> {
    if !coupon.is_tag()? || as_tree(&coupon)?.len()? != 2 {
        return Ok(None);
    }
    let author = child(&coupon, 0)?;
    if !(author.is_comparable() && author.equals(&callable()?)?) {
        let Ok(author) = as_tree(&author) else {
            return Ok(None);
        };
        if author.is_tag()? || author.len()? != 0 {
            return Ok(None);
        }
    }
    let reference = child(&coupon, 1)?;
    if reference.value_type() != ValueType::Ref {
        return Ok(None);
    }
    let Some(claim) = storage::identify(&reference)? else {
        return Ok(None);
    };
    let claim = fields(&claim)?;
    let canonical = storage::canonical_value(input)?;
    if claim.len() != 3
        || !AsValue::equals(&claim[0], operation)?
        || storage::canonical_value(&claim[1])?.raw_handle_bytes()?
            != canonical.raw_handle_bytes()?
    {
        return Ok(None);
    }
    Ok(Some(coupon))
}
async fn collect(
    policy: &(impl Scheduler + ?Sized),
    rule: Exchange,
    lhs: &Value,
    rhs: &Value,
    premises: &[Value],
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let premises = premises
        .iter()
        .cloned()
        .map(coupon)
        .collect::<Result<Vec<_>, _>>()?;
    let combination = request(rule, lhs, rhs, &premises)?;
    let coupon = kernel::invoke(&combination, &mut |advice| policy.on_advice(advice)).await?;
    let coupon = self::coupon(coupon)?;
    retain(&coupon)?;
    Ok(coupon.into_value())
}
/// Produce an Eval coupon eagerly, asking the scheduler for dependency coupons.
pub async fn eval(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let _evaluation = crate::progress::evaluation(1, input);
    if input.value_type() == ValueType::Encode {
        let thunk = crate::executor::scheduler::thunk(policy, input).await?;
        let forced = Box::pin(policy.force(&thunk)).await?;
        let forced_result = result(&forced)?;
        let result = if input.is_strict()? {
            policy.load(&forced_result).await?
        } else {
            shell::reference(&forced_result)?
        };
        let evaluated = Box::pin(policy.eval(&result)).await?;
        let eq = collect(
            policy,
            Exchange::ForceToEncode,
            input,
            &result,
            &[forced],
            retain,
        )
        .await?;
        let sym = collect(policy, Exchange::Symmetry, &result, input, &[eq], retain).await?;
        collect(
            policy,
            Exchange::EvalEq,
            input,
            &self::result(&evaluated)?,
            &[evaluated, sym],
            retain,
        )
        .await
    } else if input.is_value()? {
        produced(crate::storage::eval(input)?, retain)
    } else {
        let data = policy.load(input).await?;
        let mut coupons = Vec::new();
        let children = fields(&data)?;
        kernel::tree_progress(input, &children, 0, children.len() as u64)?;
        for value in &children {
            coupons.push(Box::pin(policy.eval(value)).await?);
            kernel::tree_progress(input, &[], coupons.len() as u64, children.len() as u64)?;
        }
        eval_tree(policy, input, &coupons, retain).await
    }
}
/// Produce a Think coupon eagerly, asking the scheduler for dependency coupons.
pub async fn think(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let _evaluation = crate::progress::evaluation(4, input);
    let data = policy.load(input).await?;
    let kind = input.unsound_thunk_type()?;
    if kind == ThunkType::Identification {
        return collect(
            policy,
            Exchange::Think,
            input,
            &shell::reference(&data)?,
            &[],
            retain,
        )
        .await;
    }
    let evaluated = Box::pin(policy.eval(&data)).await?;
    let result = result(&evaluated)?;
    let coupon = match kind {
        ThunkType::Application => Box::pin(policy.apply(&result)).await?,
        ThunkType::Selection => Box::pin(policy.select(&result)).await?,
        _ => unreachable!(),
    };
    collect(
        policy,
        Exchange::Think,
        input,
        &self::result(&coupon)?,
        &[evaluated, coupon],
        retain,
    )
    .await
}
/// Produce a Force coupon eagerly, asking the scheduler for dependency coupons.
pub async fn force(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let _evaluation = crate::progress::evaluation(3, input);
    let thought = Box::pin(policy.think(input)).await?;
    let result = result(&thought)?;
    let kind = result.value_type();
    if matches!(kind, ValueType::Object | ValueType::Ref) {
        return collect(
            policy,
            Exchange::ThinkToForce,
            input,
            &result,
            &[thought],
            retain,
        )
        .await;
    }
    let next = crate::executor::scheduler::thunk(policy, &result).await?;
    let rule = if kind == ValueType::Thunk {
        Exchange::ThunkEq
    } else if result.is_strict()? {
        Exchange::ThunkStrictEq
    } else {
        Exchange::ThunkShallowEq
    };
    let forced = Box::pin(policy.force(&next)).await?;
    let eq = collect(policy, rule, input, &next, &[thought], retain).await?;
    let sym = collect(policy, Exchange::Symmetry, &next, input, &[eq], retain).await?;
    collect(
        policy,
        Exchange::ForceEq,
        input,
        &self::result(&forced)?,
        &[sym, forced],
        retain,
    )
    .await
}
/// Run an application and pass its Apply coupon to the supplied retention callback.
pub async fn apply(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let _evaluation = crate::progress::evaluation(2, input);
    produced(
        crate::kernel::apply(input, &mut |advice| policy.on_advice(advice)).await?,
        retain,
    )
}
/// Request a Select coupon and pass it to the supplied retention callback.
pub async fn select(
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let _evaluation = crate::progress::evaluation(6, input);
    produced(crate::storage::select(input)?, retain)
}

/// Exchange the supplied child Eval coupons for a tree Eval coupon.
pub async fn eval_tree(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    coupons: &[Value],
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let results = coupons.iter().map(result).collect::<Result<Vec<_>, _>>()?;
    collect(
        policy,
        Exchange::Eval,
        input,
        &Tree::new(&results),
        coupons,
        retain,
    )
    .await
}

/// Check a coupon Tag and its locally available claim.
pub fn coupon(value: Value) -> Result<Tree, Error> {
    let coupon = Tree::try_from(value)?;
    if !coupon.is_tag() || coupon.len() != 2 {
        return Err(Error::WrongType);
    }
    if claim(&coupon)?.len() != 3 {
        return Err(Error::WrongType);
    }
    Ok(coupon)
}

/// Load the claim referred to by a coupon.
pub fn claim(coupon: &Value) -> Result<Tree, Error> {
    Tree::try_from(storage::identify(&child(coupon, 1)?)?.ok_or(Error::OutOfBounds)?)
        .map_err(Into::into)
}

/// Read the result named in a coupon claim.
pub fn result(coupon: &Value) -> Result<Value, Error> {
    claim(coupon)?.get(2)?.ok_or(Error::OutOfBounds)
}
