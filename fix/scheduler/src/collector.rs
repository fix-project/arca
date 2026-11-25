use crate::executor::Scheduler;
use crate::kernel;
use crate::shell::{self, Value, as_tree, capture, child, fields};
use crate::storage;
use alloc::{boxed::Box, vec::Vec};
pub use fixwasm::coupon::Exchange;
use fixwasm::{DataType, Error, ThunkType, Value as AsValue, ValueType, backend::Tree, tree};

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
    let mut operands = None;
    if matches!(rule, Exchange::Eval | Exchange::Congruence)
        && !coupons.is_empty()
        && let (Ok(lhs), Ok(rhs)) = (as_tree(lhs), as_tree(rhs))
    {
        if lhs.len()? != coupons.len() as u64 || rhs.len()? != coupons.len() as u64 {
            return Err(Error::WrongType);
        }
        operands = Some((lhs, rhs));
    }
    let mut canonicalize = false;
    let mut witnesses = Vec::with_capacity(coupons.len());
    let collector = callable()?;
    let collector_handle = collector.capture()?;
    for (index, coupon) in coupons.iter().enumerate() {
        let reference = child(coupon, 1)?;
        let claim = storage::identify(&reference)?.ok_or(Error::OutOfBounds)?;
        if !canonicalize {
            let author = child(coupon, 0)?.capture()?;
            let claim = as_tree(&claim)?;
            if claim.len()? != 3 {
                return Err(Error::WrongType);
            }
            canonicalize = !reference.capture()?.same_handle_bytes(&claim.reference())
                || !(author.same_handle_bytes(&collector_handle)
                    || author.value_type()? == ValueType::Object
                        && author.data_type()? == DataType::Tree
                        && author.len()? == 0
                        && !author.is_tag()?);
            if let Some((lhs, rhs)) = &operands
                && !canonicalize
            {
                canonicalize = !lhs.get(index).same_handle_bytes(&claim.get(1))
                    || !rhs.get(index).same_handle_bytes(&claim.get(2));
            }
        }
        witnesses.push(capture(tree![coupon, claim])?);
    }
    if operands.is_none() && !canonicalize {
        canonicalize = !same_exchange_operands(rule, lhs, rhs, &witnesses)?;
    }
    let request = capture(tree![
        512u64 << 20,
        &collector,
        rule as u64,
        lhs,
        rhs,
        fixwasm::Tree::from_iter(&witnesses)
    ])?;
    if canonicalize {
        storage::canonical_value(&request)
    } else {
        Ok(request)
    }
}

fn same_exchange_operands(
    rule: Exchange,
    lhs: &Value,
    rhs: &Value,
    witnesses: &[Value],
) -> Result<bool, Error> {
    let lhs = lhs.capture()?;
    let rhs = rhs.capture()?;
    let claim = |index: usize| as_tree(&child(&witnesses[index], 1)?);
    Ok(match (rule, witnesses.len()) {
        (Exchange::Reflexivity, 0) => lhs.same_handle_bytes(&rhs),
        (Exchange::Eval, 0) if lhs.data_type() != Ok(DataType::Tree) => lhs.same_handle_bytes(&rhs),
        (Exchange::Think, 0) if lhs.unsound_thunk_type()? == ThunkType::Identification => {
            lhs.same_handle_bytes(&rhs.identification())
        }
        (Exchange::Think, 2) => {
            let evaluated = claim(0)?;
            let applied = claim(1)?;
            let input = evaluated.get(1);
            let thunk = match lhs.unsound_thunk_type()? {
                ThunkType::Application => input.application(),
                ThunkType::Selection => input.selection(),
                ThunkType::Identification => return Ok(true),
            };
            lhs.same_handle_bytes(&thunk)
                && evaluated.get(2).same_handle_bytes(&applied.get(1))
                && rhs.same_handle_bytes(&applied.get(2))
        }
        (Exchange::ThinkToForce | Exchange::ThunkEq | Exchange::EvalResultEq, 1) => {
            let premise = claim(0)?;
            lhs.same_handle_bytes(&premise.get(1)) && rhs.same_handle_bytes(&premise.get(2))
        }
        (Exchange::ThunkStrictEq | Exchange::ThunkShallowEq, 1) => {
            let premise = claim(0)?;
            let encoded = if rule == Exchange::ThunkStrictEq {
                rhs.strict()
            } else {
                rhs.shallow()
            };
            lhs.same_handle_bytes(&premise.get(1)) && encoded.same_handle_bytes(&premise.get(2))
        }
        (Exchange::ForceToEncode, 1) => {
            let premise = claim(0)?;
            let thunk = premise.get(1);
            let result = premise.get(2);
            if lhs.is_strict()? {
                lhs.same_handle_bytes(&thunk.strict())
                    && if result.value_type()? == ValueType::Ref {
                        rhs.reference().same_handle_bytes(&result)
                    } else {
                        rhs.same_handle_bytes(&result)
                    }
            } else {
                lhs.same_handle_bytes(&thunk.shallow())
                    && if result.value_type()? == ValueType::Object {
                        rhs.same_handle_bytes(&result.reference())
                    } else {
                        rhs.same_handle_bytes(&result)
                    }
            }
        }
        (Exchange::Symmetry, 1) => {
            let premise = claim(0)?;
            lhs.same_handle_bytes(&premise.get(2)) && rhs.same_handle_bytes(&premise.get(1))
        }
        (Exchange::EvalEq | Exchange::ForceEq, 2) => {
            let first = claim(0)?;
            let second = claim(1)?;
            let (lhs_claim, rhs_claim) = if rule == Exchange::EvalEq {
                (&second, &first)
            } else {
                (&first, &second)
            };
            first.get(1).same_handle_bytes(&second.get(1))
                && lhs.same_handle_bytes(&lhs_claim.get(2))
                && rhs.same_handle_bytes(&rhs_claim.get(2))
        }
        _ => true,
    })
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
    if claim.len() != 3 || !AsValue::equals(&claim[0], operation)? {
        return Ok(None);
    }
    if !claim[1].capture()?.same_handle_bytes(&input.capture()?)
        && storage::canonical_value(&claim[1])?.raw_handle_bytes()?
            != storage::canonical_value(input)?.raw_handle_bytes()?
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
    if policy.discovering() {
        return core::future::pending().await;
    }
    let premises = premises
        .iter()
        .cloned()
        .map(coupon)
        .collect::<Result<Vec<_>, _>>()?;
    let combination = request(rule, lhs, rhs, &premises)?;
    let mut on_advice = |advice: &kernel::Advice| policy.on_advice(advice);
    let coupon = self::coupon(kernel::invoke(&combination, &mut on_advice).await?)?;
    retain(&coupon)?;
    Ok(coupon.into_value())
}

async fn rebase_input(
    policy: &(impl Scheduler + ?Sized),
    rule: Exchange,
    input: &Value,
    coupon: Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let proved_input = child(&claim(&coupon)?.into_value(), 1)?;
    if input.same_handle_bytes(&proved_input) || policy.discovering() {
        return Ok(coupon);
    }

    let equality = storage::canonicalize(input)?;
    if !proved_input.same_handle_bytes(&result(&equality)?) {
        return Ok(coupon);
    }
    let equality = collect(
        policy,
        Exchange::Symmetry,
        &proved_input,
        input,
        &[equality],
        retain,
    )
    .await?;
    let output = result(&coupon)?;
    let premises = match rule {
        Exchange::EvalEq => [coupon, equality],
        Exchange::ForceEq => [equality, coupon],
        _ => unreachable!(),
    };
    collect(policy, rule, input, &output, &premises, retain).await
}

/// Produce an Eval coupon eagerly, asking the scheduler for dependency coupons.
pub async fn eval(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let evaluation = crate::progress::evaluation(1, input);
    let result = async {
        if input.value_type() == ValueType::Encode {
            let thunk = storage::thunk(input)?;
            let forced = Box::pin(policy.force(&thunk)).await?;
            let forced = rebase_input(policy, Exchange::ForceEq, &thunk, forced, retain).await?;
            let forced_result = result(&forced)?;
            let result = if input.is_strict()? {
                policy.load(&forced_result).await?
            } else {
                shell::reference(&forced_result)?
            };
            let evaluated = Box::pin(policy.eval(&result)).await?;
            let evaluated =
                rebase_input(policy, Exchange::EvalEq, &result, evaluated, retain).await?;
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
            if input.value_type() == ValueType::Object {
                policy.load(input).await?;
            }
            produced(crate::storage::eval(input)?, retain)
        } else {
            let data = policy.load(input).await?;
            let mut coupons = Vec::new();
            let children = fields(&data)?;
            for value in &children {
                coupons.push(Box::pin(policy.eval(value)).await?);
            }
            eval_tree(policy, input, &coupons, retain).await
        }
    }
    .await;
    evaluation.finish(result)
}
/// Produce a Think coupon eagerly, asking the scheduler for dependency coupons.
pub async fn think(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let evaluation = crate::progress::evaluation(4, input);
    let result = async {
        let kind = input.unsound_thunk_type()?;
        if kind == ThunkType::Identification {
            return collect(
                policy,
                Exchange::Think,
                input,
                &storage::reference(input)?,
                &[],
                retain,
            )
            .await;
        }
        let data = policy.load(input).await?;
        let evaluated = Box::pin(policy.eval(&data)).await?;
        let evaluated = rebase_input(policy, Exchange::EvalEq, &data, evaluated, retain).await?;
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
    .await;
    evaluation.finish(result)
}
/// Produce a Force coupon eagerly, asking the scheduler for dependency coupons.
pub async fn force(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let evaluation = crate::progress::evaluation(3, input);
    let result = async {
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
        let next = storage::thunk(&result)?;
        let rule = if kind == ValueType::Thunk {
            Exchange::ThunkEq
        } else if result.is_strict()? {
            Exchange::ThunkStrictEq
        } else {
            Exchange::ThunkShallowEq
        };
        let forced = Box::pin(policy.force(&next)).await?;
        let forced = rebase_input(policy, Exchange::ForceEq, &next, forced, retain).await?;
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
    .await;
    evaluation.finish(result)
}
/// Run an application and pass its Apply coupon to the supplied retention callback.
pub async fn apply(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let evaluation = crate::progress::evaluation(2, input);
    let result = async { produced(policy.apply_coupon(input).await?, retain) }.await;
    evaluation.finish(result)
}
/// Request a Select coupon and pass it to the supplied retention callback.
pub async fn select(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let evaluation = crate::progress::evaluation(6, input);
    let result = async {
        let target = Tree::try_from(input.clone())?
            .get(0)?
            .ok_or(Error::OutOfBounds)?;
        if !matches!(target.value_type(), ValueType::Object | ValueType::Ref) {
            return Err(Error::WrongType);
        }
        policy.load(&target).await?;
        produced(crate::storage::select(input)?, retain)
    }
    .await;
    evaluation.finish(result)
}

/// Exchange the supplied child Eval coupons for a tree Eval coupon.
pub async fn eval_tree(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
    coupons: &[Value],
    retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
) -> Result<Value, Error> {
    let mut premises = Vec::with_capacity(coupons.len());
    let operands = as_tree(input).ok();
    for (index, coupon) in coupons.iter().enumerate() {
        let coupon = if let Some(operands) = &operands {
            let operand = capture(operands.get(index))?;
            rebase_input(policy, Exchange::EvalEq, &operand, coupon.clone(), retain).await?
        } else {
            coupon.clone()
        };
        premises.push(coupon);
    }
    let results = premises.iter().map(result).collect::<Result<Vec<_>, _>>()?;
    collect(
        policy,
        Exchange::Eval,
        input,
        &Tree::new(&results),
        &premises,
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
