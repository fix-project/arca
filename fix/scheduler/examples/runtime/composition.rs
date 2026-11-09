use crate::advice;
use crate::{Claims, Evidence, Executor, Key, Proofs, Request, Resolved, Root, todo};
use crate::{kernel, storage};
use alloc::vec::Vec;
use fix::{Error, coupon::Exchange};

pub trait Backend: Proofs {
    fn known(&self, key: &(u64, Key)) -> Option<Evidence>;
    fn cached(&self, request: &Request) -> Result<Option<Evidence>, Error>;
    fn record(&self, request: &Request, evidence: &Evidence) -> Result<(), Error>;
    fn apply(&self, input: &Root) -> Result<Evidence, Error>;
    fn select(&self, input: &Root) -> Result<Evidence, Error>;
    fn eval(&self, input: &Root) -> Result<Evidence, Error>;
    fn object(&self, input: &Root) -> Result<Root, Error>;
}

pub struct Services<P, C, O> {
    pub proofs: P,
    pub claims: C,
    pub operations: O,
}

impl<P: Proofs, C: Claims<P>, O: Operations> Backend for Services<P, C, O> {
    fn known(&self, key: &(u64, Key)) -> Option<Evidence> {
        self.claims.known(key)
    }
    fn cached(&self, request: &Request) -> Result<Option<Evidence>, Error> {
        self.claims.cached(request, &self.proofs)
    }
    fn record(&self, request: &Request, evidence: &Evidence) -> Result<(), Error> {
        self.proofs.remember(evidence)?;
        self.claims.remember(request, evidence);
        Ok(())
    }
    fn apply(&self, input: &Root) -> Result<Evidence, Error> {
        self.operations.apply(input)
    }
    fn select(&self, input: &Root) -> Result<Evidence, Error> {
        self.operations.select(input)
    }
    fn eval(&self, input: &Root) -> Result<Evidence, Error> {
        self.operations.eval(input)
    }
    fn object(&self, input: &Root) -> Result<Root, Error> {
        self.operations.object(input)
    }
}

pub trait Acquisition {
    fn acquire(&self, reference: &Root) -> Result<Option<Root>, Error>;
}

pub struct TodoAcquisition;
impl Acquisition for TodoAcquisition {
    fn acquire(&self, reference: &Root) -> Result<Option<Root>, Error> {
        todo::acquire(reference)
    }
}

pub trait Operations {
    fn apply(&self, input: &Root) -> Result<Evidence, Error>;
    fn select(&self, input: &Root) -> Result<Evidence, Error>;
    fn eval(&self, input: &Root) -> Result<Evidence, Error>;
    fn object(&self, input: &Root) -> Result<Root, Error>;
}

pub struct Effects<A, H> {
    pub acquisition: A,
    pub advice: H,
}
impl<A: Acquisition, H: advice::Handler> Operations for Effects<A, H> {
    fn apply(&self, input: &Root) -> Result<Evidence, Error> {
        Evidence::invocation(advice::resume(kernel::apply(input)?, input, &self.advice)?)
    }
    fn select(&self, input: &Root) -> Result<Evidence, Error> {
        Evidence::cache(storage::select(input)?)
    }
    fn eval(&self, input: &Root) -> Result<Evidence, Error> {
        Evidence::cache(storage::eval(input)?)
    }
    fn object(&self, input: &Root) -> Result<Root, Error> {
        match storage::identify(input)? {
            Some(object) => Ok(object),
            None => self.acquisition.acquire(input)?.ok_or(Error::OutOfBounds),
        }
    }
}

pub struct Execution<B> {
    pub backend: B,
}

impl<B: Backend> Executor for Execution<B> {
    type State = crate::EvaluationState;
    fn known(&self, key: &(u64, Key)) -> Option<Evidence> {
        self.backend.known(key)
    }
    fn cached(&self, request: &Request) -> Result<Option<Evidence>, Error> {
        self.backend.cached(request)
    }
    fn remember(&self, request: &Request, evidence: &Evidence) -> Result<(), Error> {
        self.backend.record(request, evidence)
    }
    fn step(
        &self,
        request: &Request,
        state: &mut Self::State,
        resolved: &Resolved,
        missing: &mut Vec<Request>,
    ) -> Result<Option<Evidence>, Error> {
        match state.step(
            request,
            &self.backend,
            |dependency| {
                let value = resolved
                    .get(&dependency.key())
                    .cloned()
                    .or_else(|| self.backend.known(&dependency.key()));
                if value.is_none() {
                    missing.push(dependency.clone());
                }
                Ok(value)
            },
            |input| self.backend.object(input),
        )? {
            crate::EvaluationStep::Complete(value) => Ok(Some(value)),
            crate::EvaluationStep::Dependencies(_) => Ok(None),
            crate::EvaluationStep::Primitive(request) => match request.statement {
                fix::coupon::Statement::Apply => self.backend.apply(&request.input).map(Some),
                fix::coupon::Statement::Select => self.backend.select(&request.input).map(Some),
                fix::coupon::Statement::Eval => self.backend.eval(&request.input).map(Some),
                _ => Err(Error::WrongType),
            },
        }
    }
}

impl<P: Proofs, C, O> Proofs for Services<P, C, O> {
    fn accepts(&self, author: &Root) -> Result<bool, Error> {
        self.proofs.accepts(author)
    }
    fn recommended(&self, coupon: &Root) -> Result<bool, Error> {
        self.proofs.recommended(coupon)
    }
    fn remember(&self, evidence: &Evidence) -> Result<(), Error> {
        self.proofs.remember(evidence)
    }
    fn exchange(
        &self,
        rule: Exchange,
        lhs: &Root,
        rhs: &Root,
        witnesses: &[Evidence],
    ) -> Result<Evidence, Error> {
        self.proofs.exchange(rule, lhs, rhs, witnesses)
    }
}
