use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::RefCell;
use fix::{
    Error, Value, ValueType,
    coupon::{Exchange, Statement},
    map::Map,
    tree,
};
use fixscheduler::{
    collector::{Collector, Witness},
    kernel::{self, Execution},
    shell::{Root, as_tree, child, fields, root},
    storage,
};

#[derive(Clone)]
pub(super) struct Evidence {
    pub(super) witness: Witness,
    pub(super) result: Root,
    pub(super) recommended: bool,
}

pub(super) struct Scheduler {
    collector: Root,
    known: RefCell<BTreeMap<(u64, [u8; 32]), Evidence>>,
    pending: Map,
    retained: Map,
}

impl Scheduler {
    pub(super) fn new(collector: Root, pending: Map, retained: Map) -> Self {
        Self {
            collector,
            known: RefCell::new(BTreeMap::new()),
            pending,
            retained,
        }
    }

    pub(super) fn known(&self, key: &(u64, [u8; 32])) -> Option<Evidence> {
        self.known.borrow().get(key).cloned()
    }

    pub(super) fn cached(
        &self,
        statement: Statement,
        input: &Root,
    ) -> Result<Option<Evidence>, Error> {
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

    pub(super) fn remember(
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
                    if advice.key_is(fix::advice::CANONICALIZE)? {
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

    pub(super) fn primitive(&self, statement: Statement, input: &Root) -> Result<Evidence, Error> {
        let (coupon, recommended) = match statement {
            Statement::Apply => self.resume(kernel::apply(input)?, input)?,
            Statement::Select => (storage::select(input)?, false),
            Statement::Eval => (storage::eval(input)?, false),
            _ => return Err(Error::WrongType),
        };
        self.evidence(coupon, recommended)
    }

    pub(super) fn exchange(
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

pub(super) fn object(value: &Root) -> Result<Root, Error> {
    match storage::identify(value)? {
        Some(object) => Ok(object),
        None => storage::acquire(value)?.ok_or(Error::OutOfBounds),
    }
}

pub(super) fn finish(result: Evidence) -> Result<Root, Error> {
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

pub(super) fn wait() -> Result<Root, Error> {
    loop {
        let notification = kernel::worker::listen()?;
        if kernel::worker::stopped()? {
            return root(tree![]);
        }
        notification.wait()?;
    }
}

pub(super) fn todo(input: &Root) -> Result<Root, Error> {
    let action = child(input, 0)?;
    let value = child(input, 1)?;
    if action.equals(fix::Blob::create(b"import"))? {
        match storage::import_source(&value)? {
            Some(value) => root(tree![0u64, value]),
            None => root(tree![1u64, fix::Blob::create(b"could not import URI")]),
        }
    } else if action.equals(fix::Blob::create(b"add"))? {
        storage::add_sources(&value, &fields(&child(input, 2)?)?)
    } else if action.equals(fix::Blob::create(b"list"))? {
        storage::sources(&value)
    } else {
        Err(Error::WrongType)
    }
}
