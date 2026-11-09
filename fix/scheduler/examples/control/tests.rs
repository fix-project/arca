use crate::*;
use alloc::{
    collections::{BTreeMap, VecDeque},
    vec,
    vec::Vec,
};

#[derive(Default)]
struct State {
    suspended: bool,
}
fn evaluate(
    request: &u8,
    state: &mut State,
    resolved: &BTreeMap<u8, u64>,
) -> Result<Poll<u8, u8, u64>, &'static str> {
    if *request < 2 {
        return Ok(Poll::Complete(u64::from(*request) + 2));
    }
    match (resolved.get(&(request - 1)), resolved.get(&(request - 2))) {
        (Some(a), Some(b)) => {
            if !state.suspended {
                return Err("lost evaluator state");
            }
            Ok(Poll::Complete(a + b))
        }
        _ => {
            state.suspended = true;
            Ok(Poll::Blocked(vec![
                (request - 1, request - 1),
                (request - 2, request - 2),
            ]))
        }
    }
}

#[derive(Default)]
struct Mailboxes {
    commands: BTreeMap<u8, VecDeque<Command<u8, u64>>>,
    events: VecDeque<(u8, Event<u8, u64>)>,
}
impl Transport<u8, Command<u8, u64>, Event<u8, u64>> for Mailboxes {
    type Error = ();
    fn send(&mut self, destination: &u8, command: Command<u8, u64>) -> Result<(), ()> {
        self.commands
            .entry(*destination)
            .or_default()
            .push_back(command);
        Ok(())
    }
    fn receive(&mut self) -> Result<Option<(u8, Event<u8, u64>)>, ()> {
        Ok(self.events.pop_front())
    }
}

#[test]
fn centralized_placement_preserves_opaque_state_and_shared_dependencies() {
    for workers in [1, 4] {
        let mut coordinator = Coordinator::default();
        let mut owners = Ownership::default();
        let mut transport = Mailboxes::default();
        let mut tasks = BTreeMap::<u8, BTreeMap<Ticket, Task<u8, u8, u64, State>>>::new();
        let (root, ready) = coordinator.submit(8, 8).unwrap();
        let mut ready: VecDeque<_> = ready.into();
        while coordinator.result(root).is_none() {
            while let Some(command) = ready.pop_front() {
                let ticket = command.ticket();
                let destination = match &command {
                    Command::Start { request, .. } => {
                        let destination = request % workers;
                        assert!(owners.assign(ticket, destination));
                        destination
                    }
                    Command::Resume { .. } => *owners.owner(ticket).unwrap(),
                };
                transport.send(&destination, command).unwrap();
            }
            for peer in 0..workers {
                let Some(command) = transport.commands.entry(peer).or_default().pop_front() else {
                    continue;
                };
                let ticket = command.ticket();
                assert!(owners.accepts(ticket, &peer));
                let local = tasks.entry(peer).or_default();
                match command {
                    Command::Start { request, .. } => {
                        local.insert(ticket, Task::new(request));
                    }
                    Command::Resume { values, .. } => {
                        local.get_mut(&ticket).unwrap().resume(values).unwrap();
                    }
                }
                let event = match local.get_mut(&ticket).unwrap().poll(evaluate).unwrap() {
                    Step::Blocked(dependencies) => Event::Blocked {
                        ticket,
                        dependencies,
                    },
                    Step::Complete(value) => {
                        local.remove(&ticket);
                        Event::Complete { ticket, value }
                    }
                };
                transport.events.push_back((peer, event));
            }
            while let Some((peer, event)) = transport.receive().unwrap() {
                let ticket = event.ticket();
                assert!(owners.accepts(ticket, &peer));
                let commands = match event {
                    Event::Blocked { dependencies, .. } => coordinator
                        .blocked(
                            ticket,
                            dependencies
                                .into_iter()
                                .map(|(slot, request)| (slot, request, request))
                                .collect(),
                        )
                        .unwrap(),
                    Event::Complete { value, .. } => {
                        assert!(owners.release(ticket, &peer));
                        coordinator.complete(ticket, value).unwrap()
                    }
                    Event::Advice { .. } => Vec::new(),
                };
                ready.extend(commands);
            }
        }
        assert_eq!(coordinator.result(root), Some(&89));
    }
}

#[test]
fn caller_core_loops_choose_run_route_or_wait() {
    for cores in [1u8, 4] {
        let mut scheduler = Centralized::new(
            0,
            8,
            |request: &u8| *request,
            |request: &u8| request % cores,
        )
        .unwrap();
        let endpoint = Endpoint { coordinator: 0 };
        let mut inboxes: BTreeMap<u8, VecDeque<(u8, Message<u8, u64>)>> = BTreeMap::new();
        let mut local = BTreeMap::<u8, BTreeMap<Ticket, Task<u8, u8, u64, State>>>::new();
        let result = 'done: loop {
            for core in 0..cores {
                let mut input = inboxes
                    .entry(core)
                    .or_default()
                    .pop_front()
                    .map(|(source, message)| Input::Message { source, message });
                loop {
                    let decision = if core == 0 {
                        scheduler.next(&core, input).unwrap()
                    } else {
                        endpoint.next(&core, input).unwrap()
                    };
                    input = None;
                    match decision {
                        Decision::Run(command) => {
                            let ticket = command.ticket();
                            let tasks = local.entry(core).or_default();
                            match command {
                                Command::Start { request, .. } => {
                                    assert_eq!(core, request % cores);
                                    tasks.insert(ticket, Task::new(request));
                                }
                                Command::Resume { values, .. } => {
                                    tasks.get_mut(&ticket).unwrap().resume(values).unwrap()
                                }
                            }
                            let event =
                                match tasks.get_mut(&ticket).unwrap().poll(evaluate).unwrap() {
                                    Step::Blocked(dependencies) => Event::Blocked {
                                        ticket,
                                        dependencies,
                                    },
                                    Step::Complete(value) => {
                                        tasks.remove(&ticket);
                                        Event::Complete { ticket, value }
                                    }
                                };
                            input = Some(Input::Executed(event));
                        }
                        Decision::Route {
                            destination,
                            message,
                        } => inboxes
                            .entry(destination)
                            .or_default()
                            .push_back((core, message)),
                        Decision::Wait => break,
                        Decision::Complete(value) => break 'done value,
                        Decision::Advice { .. } => {}
                    }
                }
            }
        };
        assert_eq!(result, 89);
    }
}
