#![no_std]
extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::RefCell;
use fixscheduler::shell;
use fixscheduler::{
    collector::{Collector, Witness},
    kernel::{self, Execution},
    shell::{Root, as_tree, child, fields, root},
    storage,
};
use fixwasm::{
    Error, Tree, Value, ValueType,
    coupon::{Exchange, Statement},
    map::Map,
    tree,
};
use fixwasm::{Handle, Object, ThunkType};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    if kernel::worker::id()? != 0 {
        return wait();
    }
    assert_eq!(combination.len()?, 4);
    let mode = combination.get(0);
    let input = root(combination.get(3))?;
    if mode.equals(fixwasm::Blob::create(b"todo"))? {
        let result = todo(&input)?;
        kernel::worker::finish(&result)?;
        return Ok(result);
    }
    let statement = if mode.equals(fixwasm::Blob::create(b"eval"))? {
        Statement::Eval
    } else if mode.equals(fixwasm::Blob::create(b"think"))? {
        Statement::Think
    } else {
        return Err(Error::WrongType);
    };
    let scheduler = Scheduler::new(root(combination.get(2))?, Map::create()?, Map::create()?);
    finish(evaluate(&scheduler, statement, &input)?)
}

fn evaluate(scheduler: &Scheduler, statement: Statement, input: &Root) -> Result<Evidence, Error> {
    if let Some(value) = scheduler.cached(statement, input)? {
        return Ok(value);
    }
    let result = match statement {
        Statement::Apply | Statement::Select => scheduler.primitive(statement, input)?,
        Statement::Eval => {
            if input.value_type()? == ValueType::Encode {
                let thunk = thunk(input)?;
                let forced = evaluate(scheduler, Statement::Force, &thunk)?;
                let data = if input.is_strict()? {
                    object(&forced.result)?
                } else {
                    shell::reference(&forced.result)?
                };
                let evaluated = evaluate(scheduler, Statement::Eval, &data)?;
                let eq = scheduler.exchange(Exchange::ForceToEncode, input, &data, &[forced])?;
                let sym = scheduler.exchange(Exchange::Symmetry, &data, input, &[eq])?;
                scheduler.exchange(
                    Exchange::EvalEq,
                    input,
                    &evaluated.result,
                    &[evaluated.clone(), sym],
                )?
            } else if input.is_value()? {
                scheduler.primitive(statement, input)?
            } else {
                let premises = fields(input)?
                    .iter()
                    .map(|child| evaluate(scheduler, Statement::Eval, child))
                    .collect::<Result<Vec<_>, Error>>()?;
                let result = root(Tree::from_iter(
                    premises.iter().map(|evidence| &evidence.result),
                ))?;
                scheduler.exchange(Exchange::Eval, input, &result, &premises)?
            }
        }
        Statement::Think => {
            let data = object(input)?;
            match input.unsound_thunk_type()? {
                ThunkType::Identification => {
                    scheduler.exchange(Exchange::Think, input, &shell::reference(&data)?, &[])?
                }
                kind => {
                    let evaluated = evaluate(scheduler, Statement::Eval, &data)?;
                    let statement = match kind {
                        ThunkType::Application => Statement::Apply,
                        ThunkType::Selection => Statement::Select,
                        ThunkType::Identification => unreachable!(),
                    };
                    let applied = evaluate(scheduler, statement, &evaluated.result)?;
                    scheduler.exchange(
                        Exchange::Think,
                        input,
                        &applied.result,
                        &[evaluated, applied.clone()],
                    )?
                }
            }
        }
        Statement::Force => {
            let thought = evaluate(scheduler, Statement::Think, input)?;
            let kind = thought.result.value_type()?;
            if matches!(kind, ValueType::Object | ValueType::Ref) {
                scheduler.exchange(
                    Exchange::ThinkToForce,
                    input,
                    &thought.result,
                    &[thought.clone()],
                )?
            } else {
                let next = thunk(&thought.result)?;
                let forced = evaluate(scheduler, Statement::Force, &next)?;
                let rule = if kind == ValueType::Thunk {
                    Exchange::ThunkEq
                } else if thought.result.is_strict()? {
                    Exchange::ThunkStrictEq
                } else {
                    Exchange::ThunkShallowEq
                };
                let eq = scheduler.exchange(rule, input, &next, &[thought])?;
                let sym = scheduler.exchange(Exchange::Symmetry, &next, input, &[eq])?;
                scheduler.exchange(
                    Exchange::ForceEq,
                    input,
                    &forced.result,
                    &[sym, forced.clone()],
                )?
            }
        }
        _ => return Err(Error::WrongType),
    };
    scheduler.remember(statement, input, &result)?;
    Ok(result)
}

fn thunk(value: &Root) -> Result<Root, Error> {
    let data = object(value)?;
    match value.unsound_thunk_type()? {
        ThunkType::Application => root(as_tree(&data)?.application()),
        ThunkType::Identification => root(data.identification()),
        ThunkType::Selection => root(as_tree(&data)?.selection()),
    }
}

#[derive(Clone)]
struct Evidence {
    witness: Witness,
    result: Root,
    recommended: bool,
}

struct Scheduler {
    collector: Root,
    known: RefCell<BTreeMap<(u64, [u8; 32]), Evidence>>,
    pending: Map,
    retained: Map,
}

impl Scheduler {
    fn new(collector: Root, pending: Map, retained: Map) -> Self {
        Self {
            collector,
            known: RefCell::new(BTreeMap::new()),
            pending,
            retained,
        }
    }

    fn known(&self, key: &(u64, [u8; 32])) -> Option<Evidence> {
        self.known.borrow().get(key).cloned()
    }

    fn cached(&self, statement: Statement, input: &Root) -> Result<Option<Evidence>, Error> {
        let key = (statement as u64, input.raw_handle_bytes()?);
        if let Some(evidence) = self.known(&key) {
            return Ok(Some(evidence));
        }
        for coupon in fields(&storage::claims(statement, input)?)? {
            if !coupon.is_tag()? || as_tree(&coupon)?.len()? != 2 {
                continue;
            }
            let author = child(&coupon, 0)?;
            if !(author.is_comparable()? && author.equals(&self.collector)?) {
                let Ok(kernel) = as_tree(&author) else {
                    continue;
                };
                if kernel.is_tag()? || kernel.len()? != 0 {
                    continue;
                }
            }
            let claim = object(&child(&coupon, 1)?)?;
            if as_tree(&claim)?.len()? != 3 {
                continue;
            }
            let evidence = Evidence {
                result: child(&claim, 2)?,
                recommended: self.retained.get(&coupon)?.is_some(),
                witness: Witness {
                    coupon,
                    assertion: claim,
                },
            };
            self.known.borrow_mut().insert(key, evidence.clone());
            return Ok(Some(evidence));
        }
        Ok(None)
    }

    fn remember(
        &self,
        statement: Statement,
        input: &Root,
        evidence: &Evidence,
    ) -> Result<(), Error> {
        if evidence.recommended {
            self.retained
                .compare_exchange(&evidence.witness.coupon, 0, tree![])?;
        }
        self.known.borrow_mut().insert(
            (statement as u64, input.raw_handle_bytes()?),
            evidence.clone(),
        );
        Ok(())
    }

    fn resume(&self, mut execution: Execution, input: &Root) -> Result<(Root, bool), Error> {
        loop {
            match execution {
                Execution::Complete(coupon) => {
                    return Ok((coupon, self.pending.remove(input)?.is_some()));
                }
                Execution::Advice {
                    advice,
                    continuation,
                } => {
                    if advice.key_is(fixwasm::advice::CANONICALIZE)? {
                        let target = if matches!(
                            advice.target.value_type()?,
                            ValueType::Thunk | ValueType::Ref
                        ) {
                            storage::identify(&advice.target)?
                        } else {
                            Some(advice.target)
                        };
                        if let Some(target) = target {
                            self.pending.compare_exchange(target, 0, tree![])?;
                        }
                    }
                    execution = continuation.resume()?;
                }
            }
        }
    }

    fn evidence(&self, coupon: Root, recommended: bool) -> Result<Evidence, Error> {
        let claim = object(&child(&coupon, 1)?)?;
        let result = child(&claim, 2)?;
        if recommended {
            storage::recommend(&coupon)?;
        } else {
            storage::cache(&coupon, false)?;
        }
        Ok(Evidence {
            witness: Witness {
                coupon,
                assertion: claim,
            },
            result,
            recommended,
        })
    }

    fn primitive(&self, statement: Statement, input: &Root) -> Result<Evidence, Error> {
        let (coupon, recommended) = match statement {
            Statement::Apply => self.resume(kernel::apply(input)?, input)?,
            Statement::Select => (storage::select(input)?, false),
            Statement::Eval => (storage::eval(input)?, false),
            _ => return Err(Error::WrongType),
        };
        self.evidence(coupon, recommended)
    }

    fn exchange(
        &self,
        rule: Exchange,
        lhs: &Root,
        rhs: &Root,
        premises: &[Evidence],
    ) -> Result<Evidence, Error> {
        let witnesses: Vec<_> = premises
            .iter()
            .map(|evidence| evidence.witness.clone())
            .collect();
        let combination =
            Collector::new(self.collector.clone()).request(rule, lhs, rhs, &witnesses)?;
        let (coupon, direct) = self.resume(kernel::invoke(&combination)?, &combination)?;
        let inherited = match rule {
            Exchange::Think | Exchange::EvalEq => premises.get(1),
            Exchange::ThinkToForce
            | Exchange::ForceToEncode
            | Exchange::Symmetry
            | Exchange::ThunkEq
            | Exchange::ThunkStrictEq
            | Exchange::ThunkShallowEq
            | Exchange::ForceEq => premises.first(),
            _ => None,
        };
        self.evidence(
            coupon,
            direct || inherited.is_some_and(|evidence| evidence.recommended),
        )
    }
}

fn object(value: &Root) -> Result<Root, Error> {
    match storage::identify(value)? {
        Some(object) => Ok(object),
        None => storage::acquire(value)?.ok_or(Error::OutOfBounds),
    }
}

fn finish(result: Evidence) -> Result<Root, Error> {
    let result = if kernel::worker::returns_coupon()? {
        let coupon = result.witness.coupon;
        storage::cache(&coupon, true)?;
        coupon
    } else {
        result.result
    };
    kernel::worker::finish(&result)?;
    Ok(result)
}

fn wait() -> Result<Root, Error> {
    loop {
        let notification = kernel::worker::listen()?;
        if kernel::worker::stopped()? {
            return root(tree![]);
        }
        notification.wait()?;
    }
}

fn todo(input: &Root) -> Result<Root, Error> {
    let action = child(input, 0)?;
    let value = child(input, 1)?;
    if action.equals(fixwasm::Blob::create(b"import"))? {
        match storage::import_source(&value)? {
            Some(value) => root(tree![0u64, value]),
            None => root(tree![1u64, fixwasm::Blob::create(b"could not import URI")]),
        }
    } else if action.equals(fixwasm::Blob::create(b"add"))? {
        storage::add_sources(&value, &fields(&child(input, 2)?)?)
    } else if action.equals(fixwasm::Blob::create(b"list"))? {
        storage::sources(&value)
    } else {
        Err(Error::WrongType)
    }
}
