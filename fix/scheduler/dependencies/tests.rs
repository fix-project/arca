use crate::*;
use alloc::{collections::BTreeMap, vec};

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

#[test]
fn task_can_be_driven_without_a_scheduler() {
    let mut task = Task::<u8, u8, u64, State>::new(2);
    let Step::Blocked(dependencies) = task.poll(evaluate).unwrap() else {
        panic!("expected dependencies")
    };
    let responses = dependencies
        .into_iter()
        .map(|(slot, request)| (slot, u64::from(request) + 2))
        .collect();
    task.resume(responses).unwrap();
    assert!(matches!(task.poll(evaluate).unwrap(), Step::Complete(5)));
}
