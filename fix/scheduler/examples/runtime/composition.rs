use crate::advice;
use crate::effects::{self, callable, storage};
use crate::{Claims, Evidence, Executor, Key, Proofs, Request, Resolved, Root, todo};
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
        Evidence::invocation(callable::apply(input, &self.advice)?)
    }
    fn select(&self, input: &Root) -> Result<Evidence, Error> {
        Evidence::cache(storage::select(input)?)
    }
    fn eval(&self, input: &Root) -> Result<Evidence, Error> {
        Evidence::cache(storage::eval(input)?)
    }
    fn object(&self, input: &Root) -> Result<Root, Error> {
        let (status, result) =
            effects::coordinate(b"storage_identify", core::slice::from_ref(input))?;
        match status {
            0 => Ok(result),
            1 => self.acquisition.acquire(input)?.ok_or(Error::OutOfBounds),
            _ => Err(Error::WrongType),
        }
    }
}

pub trait Evaluator {
    type State: Default;
    fn step<B: Backend>(
        &self,
        request: &Request,
        state: &mut Self::State,
        context: &mut Context<'_, B>,
    ) -> Result<Option<Evidence>, Error>;
}

pub struct Context<'a, B> {
    pub backend: &'a B,
    resolved: &'a Resolved,
    missing: &'a mut Vec<Request>,
}

impl<'a, B: Backend> Context<'a, B> {
    pub fn new(backend: &'a B, resolved: &'a Resolved, missing: &'a mut Vec<Request>) -> Self {
        Self {
            backend,
            resolved,
            missing,
        }
    }
    pub fn require(&mut self, request: &Request) -> Result<Option<Evidence>, Error> {
        let key = request.key();
        let value = self
            .resolved
            .get(&key)
            .cloned()
            .or_else(|| self.backend.known(&key));
        if value.is_none() {
            self.missing.push(request.clone());
        }
        Ok(value)
    }
}

pub struct Execution<B> {
    pub backend: B,
}

impl<B: Backend> Executor for Execution<B> {
    type State = fixscheduler::EvaluationState;
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
        FixRules.step(
            request,
            state,
            &mut Context {
                backend: &self.backend,
                resolved,
                missing,
            },
        )
    }
}

pub struct FixRules;
impl Evaluator for FixRules {
    type State = fixscheduler::EvaluationState;
    fn step<B: Backend>(
        &self,
        request: &Request,
        state: &mut Self::State,
        context: &mut Context<'_, B>,
    ) -> Result<Option<Evidence>, Error> {
        let backend = context.backend;
        match state.step(
            request,
            backend,
            |dependency| context.require(dependency),
            |input| backend.object(input),
        )? {
            fixscheduler::EvaluationStep::Complete(value) => Ok(Some(value)),
            fixscheduler::EvaluationStep::Dependencies(_) => Ok(None),
            fixscheduler::EvaluationStep::Primitive(request) => match request.statement {
                fix::coupon::Statement::Apply => backend.apply(&request.input).map(Some),
                fix::coupon::Statement::Select => backend.select(&request.input).map(Some),
                fix::coupon::Statement::Eval => backend.eval(&request.input).map(Some),
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
