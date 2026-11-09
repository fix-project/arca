use crate::{Command, Coordinator, Error, Event, Ownership};
use alloc::collections::VecDeque;

pub enum Message<R, V, A = ()> {
    Work(Command<R, V>),
    Report(Event<R, V, A>),
}

pub enum Input<P, R, V, A = ()> {
    Message {
        source: P,
        message: Message<R, V, A>,
    },
    Executed(Event<R, V, A>),
}

pub enum Decision<P, R, V, A = ()> {
    Run(Command<R, V>),
    Route {
        destination: P,
        message: Message<R, V, A>,
    },
    Advice {
        source: P,
        value: A,
    },
    Wait,
    Complete(V),
}

pub struct Endpoint<P> {
    pub coordinator: P,
}
impl<P: Clone> Endpoint<P> {
    pub fn next<R, V, A>(
        &self,
        core: &P,
        input: Option<Input<P, R, V, A>>,
    ) -> Result<Decision<P, R, V, A>, Error> {
        match input {
            Some(Input::Message {
                message: Message::Work(command),
                ..
            }) => Ok(Decision::Run(command)),
            Some(Input::Executed(event)) => Ok(Decision::Route {
                destination: self.coordinator.clone(),
                message: Message::Report(event),
            }),
            Some(Input::Message {
                message: Message::Report(Event::Advice { value, .. }),
                source,
            }) => Ok(Decision::Advice { source, value }),
            Some(Input::Message {
                message: Message::Report(_),
                ..
            }) => Err(Error::WrongEndpoint),
            None => {
                let _ = core;
                Ok(Decision::Wait)
            }
        }
    }
}

pub struct Centralized<P, R, K, V, I, F> {
    core: P,
    root: u64,
    dependencies: Coordinator<R, K, V>,
    ownership: Ownership<P>,
    ready: VecDeque<Command<R, V>>,
    identity: I,
    placement: F,
}
impl<P: Clone + Eq, R: Clone, K: Ord, V: Clone, I: FnMut(&R) -> K, F: FnMut(&R) -> P>
    Centralized<P, R, K, V, I, F>
{
    pub fn new(core: P, request: R, mut identity: I, placement: F) -> Result<Self, Error> {
        let mut dependencies = Coordinator::default();
        let (root, ready) = dependencies.submit(identity(&request), request)?;
        Ok(Self {
            core,
            root,
            dependencies,
            ownership: Ownership::default(),
            ready: ready.into(),
            identity,
            placement,
        })
    }

    pub fn request(&self, ticket: u64) -> Result<&R, Error> {
        self.dependencies.request(ticket)
    }

    pub fn next<A>(
        &mut self,
        core: &P,
        input: Option<Input<P, R, V, A>>,
    ) -> Result<Decision<P, R, V, A>, Error> {
        match input {
            Some(Input::Message {
                message: Message::Work(command),
                ..
            }) => return Ok(Decision::Run(command)),
            Some(Input::Executed(event)) => {
                return Ok(Decision::Route {
                    destination: self.core.clone(),
                    message: Message::Report(event),
                });
            }
            Some(Input::Message {
                source,
                message: Message::Report(event),
            }) => {
                if core != &self.core || !self.ownership.accepts(event.ticket(), &source) {
                    return Err(Error::WrongEndpoint);
                }
                let ticket = event.ticket();
                match event {
                    Event::Blocked { dependencies, .. } => {
                        let dependencies = dependencies
                            .into_iter()
                            .map(|(slot, request)| (slot, (self.identity)(&request), request))
                            .collect();
                        self.ready
                            .extend(self.dependencies.blocked(ticket, dependencies)?);
                    }
                    Event::Complete { value, .. } => {
                        self.ownership.release(ticket, &source);
                        self.ready
                            .extend(self.dependencies.complete(ticket, value)?);
                    }
                    Event::Advice { value, .. } => return Ok(Decision::Advice { source, value }),
                }
            }
            None => {}
        }
        if core != &self.core {
            return Ok(Decision::Wait);
        }
        if let Some(result) = self.dependencies.result(self.root) {
            return Ok(Decision::Complete(result.clone()));
        }
        let Some(command) = self.ready.pop_front() else {
            return Ok(Decision::Wait);
        };
        let ticket = command.ticket();
        let destination = if let Some(owner) = self.ownership.owner(ticket) {
            owner.clone()
        } else {
            let destination = (self.placement)(self.dependencies.request(ticket)?);
            self.ownership.assign(ticket, destination.clone());
            destination
        };
        if &destination == core {
            Ok(Decision::Run(command))
        } else {
            Ok(Decision::Route {
                destination,
                message: Message::Work(command),
            })
        }
    }
}
