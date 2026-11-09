use super::effects::{callable, storage};
use super::{Key, Resolved, Root, as_tree, child, fields, root};
use crate::Request;
use core::cell::RefCell;
use fix::coupon::Exchange;
use fix::map::Map;
use fix::{Error, Tree, Value, tree};

#[derive(Clone)]
pub struct Evidence {
    pub witness: Root,
    pub result: Root,
    pub recommended: bool,
}

impl Evidence {
    pub fn admit(coupon: Root, request: &Request, proofs: &impl Proofs) -> Result<Self, Error> {
        if !coupon.is_tag()?
            || as_tree(&coupon)?.len()? != 2
            || !proofs.accepts(&child(&coupon, 0)?)?
        {
            return Err(Error::WrongType);
        }
        let evidence = Self::from_coupon(coupon)?;
        let claim = child(&evidence.witness, 1)?;
        if child(&claim, 0)?.read_u64()? != request.statement as u64
            || child(&claim, 1)?.raw_handle_bytes()? != request.input.raw_handle_bytes()?
        {
            return Err(Error::WrongType);
        }
        Ok(evidence)
    }

    pub fn cache(coupon: Root) -> Result<Self, Error> {
        Self::invocation(crate::advice::Invocation {
            value: coupon,
            recommended: false,
        })
    }

    pub fn invocation(invocation: crate::advice::Invocation) -> Result<Self, Error> {
        let mut evidence = Self::from_coupon(invocation.value.clone())?;
        evidence.recommended = invocation.recommended;
        if evidence.recommended {
            storage::recommend(&invocation.value)?;
        } else {
            storage::cache(&invocation.value, false)?;
        }
        Ok(evidence)
    }

    pub fn from_coupon(coupon: Root) -> Result<Self, Error> {
        let claim = storage::object(&child(&coupon, 1)?)?;
        Self::from_claim(coupon, claim)
    }

    fn from_claim(coupon: Root, claim: Root) -> Result<Self, Error> {
        let result = child(&claim, 2)?;
        Ok(Self {
            witness: root(tree![&coupon, &claim])?,
            result,
            recommended: false,
        })
    }
}

pub trait Proofs {
    fn accepts(&self, author: &Root) -> Result<bool, Error>;
    fn recommended(&self, coupon: &Root) -> Result<bool, Error>;
    fn remember(&self, evidence: &Evidence) -> Result<(), Error>;
    fn exchange(
        &self,
        rule: Exchange,
        lhs: &Root,
        rhs: &Root,
        witnesses: &[Evidence],
    ) -> Result<Evidence, Error>;
}

pub trait Claims<P: Proofs> {
    fn known(&self, key: &(u64, Key)) -> Option<Evidence>;
    fn remember(&self, request: &Request, evidence: &Evidence);
    fn cached(&self, request: &Request, proofs: &P) -> Result<Option<Evidence>, Error>;
}

#[derive(Default)]
pub struct CachedClaims {
    known: RefCell<Resolved>,
}

impl<P: Proofs> Claims<P> for CachedClaims {
    fn known(&self, key: &(u64, Key)) -> Option<Evidence> {
        self.known.borrow().get(key).cloned()
    }
    fn remember(&self, request: &Request, evidence: &Evidence) {
        self.known
            .borrow_mut()
            .insert(request.key(), evidence.clone());
    }
    fn cached(&self, request: &Request, proofs: &P) -> Result<Option<Evidence>, Error> {
        if let Some(evidence) = self.known.borrow().get(&request.key()).cloned() {
            return Ok(Some(evidence));
        }
        let coupons = storage::claims(request.statement, &request.input)?;
        for coupon in fields(&coupons)? {
            if !coupon.is_tag()? || as_tree(&coupon)?.len()? != 2 {
                continue;
            }
            if !proofs.accepts(&child(&coupon, 0)?)? {
                continue;
            }
            let claim = storage::object(&child(&coupon, 1)?)?;
            if as_tree(&claim)?.len()? == 3 {
                let recommended = proofs.recommended(&coupon)?;
                let mut evidence = Evidence::from_claim(coupon, claim)?;
                evidence.recommended = recommended;
                self.known
                    .borrow_mut()
                    .insert(request.key(), evidence.clone());
                return Ok(Some(evidence));
            }
        }
        Ok(None)
    }
}

pub trait Retention {
    fn recommend(&self, rule: Exchange, direct: bool, premises: &[Evidence]) -> bool;
    fn cached(&self, coupon: &Root) -> Result<bool, Error>;
    fn remember(&self, coupon: &Root) -> Result<(), Error>;
}

pub struct DirectRetention;
impl Retention for DirectRetention {
    fn recommend(&self, _: Exchange, direct: bool, _: &[Evidence]) -> bool {
        direct
    }
    fn cached(&self, _: &Root) -> Result<bool, Error> {
        Ok(false)
    }
    fn remember(&self, _: &Root) -> Result<(), Error> {
        Ok(())
    }
}

pub struct InputRetention {
    recommended: Map,
}
impl InputRetention {
    pub fn new(recommended: Map) -> Self {
        Self { recommended }
    }
}
impl Retention for InputRetention {
    fn recommend(&self, rule: Exchange, direct: bool, premises: &[Evidence]) -> bool {
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
        direct || inherited.is_some_and(|premise| premise.recommended)
    }
    fn cached(&self, coupon: &Root) -> Result<bool, Error> {
        Ok(self.recommended.get(coupon)?.is_some())
    }
    fn remember(&self, coupon: &Root) -> Result<(), Error> {
        self.recommended.compare_exchange(coupon, 0, tree![])?;
        Ok(())
    }
}

pub struct Collector<H, R> {
    callable: Root,
    advice: H,
    retention: R,
}
impl<H, R> Collector<H, R> {
    pub fn new(callable: Root, advice: H, retention: R) -> Self {
        Self {
            callable,
            advice,
            retention,
        }
    }
}
impl<H: crate::advice::Handler, R: Retention> Proofs for Collector<H, R> {
    fn accepts(&self, author: &Root) -> Result<bool, Error> {
        if author.is_comparable()? && author.equals(&*self.callable)? {
            return Ok(true);
        }
        let Ok(kernel) = as_tree(author) else {
            return Ok(false);
        };
        Ok(!kernel.is_tag()? && kernel.len()? == 0)
    }
    fn recommended(&self, coupon: &Root) -> Result<bool, Error> {
        self.retention.cached(coupon)
    }
    fn remember(&self, evidence: &Evidence) -> Result<(), Error> {
        if evidence.recommended {
            self.retention.remember(&child(&evidence.witness, 0)?)?;
        }
        Ok(())
    }
    fn exchange(
        &self,
        rule: Exchange,
        lhs: &Root,
        rhs: &Root,
        witnesses: &[Evidence],
    ) -> Result<Evidence, Error> {
        let combination = root(tree![
            0u64,
            &self.callable,
            rule as u64,
            &lhs,
            &rhs,
            Tree::from_iter(witnesses.iter().map(|evidence| &evidence.witness))
        ])?;
        let mut invocation = callable::invoke(&combination, &self.advice)?;
        invocation.recommended = self
            .retention
            .recommend(rule, invocation.recommended, witnesses);
        Evidence::invocation(invocation)
    }
}
