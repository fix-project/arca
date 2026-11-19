extern crate alloc;
#[path = "../src/executor/task.rs"]
mod task;

use std::{
    cell::{Cell, RefCell},
    future::poll_fn,
    rc::Rc,
    task::{Poll, Waker},
};
use task::Task;

#[test]
fn independent_tasks_are_polled_only_when_woken() {
    let counts = [Cell::new(0), Cell::new(0)];
    let wakers: [RefCell<Option<Waker>>; 2] = Default::default();
    let ready = [Cell::new(false), Cell::new(false)];
    let future = |index: usize| {
        let (counts, wakers, ready) = (&counts, &wakers, &ready);
        poll_fn(move |cx| {
            counts[index].set(counts[index].get() + 1);
            wakers[index].replace(Some(cx.waker().clone()));
            if ready[index].get() {
                Poll::Ready(index)
            } else {
                Poll::Pending
            }
        })
    };
    let mut first = Task::new(future(0));
    let mut second = Task::new(future(1));
    assert!(first.poll().is_pending());
    assert!(second.poll().is_pending());
    ready[1].set(true);
    wakers[1].borrow().as_ref().unwrap().wake_by_ref();
    wakers[1].borrow().as_ref().unwrap().wake_by_ref();
    assert!(first.poll().is_pending());
    assert_eq!(second.poll(), Poll::Ready(1));
    assert_eq!([counts[0].get(), counts[1].get()], [1, 2]);
}

#[test]
fn self_wake_and_stale_waker_do_not_lose_or_misdirect_work() {
    let old_waker = RefCell::new(None);
    let polls = Cell::new(0);
    let mut task = Task::new(poll_fn(|cx| {
        polls.set(polls.get() + 1);
        old_waker.replace(Some(cx.waker().clone()));
        if polls.get() == 1 {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }));
    assert!(task.poll().is_pending());
    assert!(task.is_ready());
    assert!(task.poll().is_ready());
    drop(task);
    let dropped = Rc::new(Cell::new(false));
    struct OnDrop(Rc<Cell<bool>>);
    impl Drop for OnDrop {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let guard = OnDrop(dropped.clone());
    let mut replacement = Task::new(async move {
        let _guard = guard;
        std::future::pending::<()>().await;
    });
    assert!(replacement.poll().is_pending());
    old_waker.borrow().as_ref().unwrap().wake_by_ref();
    assert!(!replacement.is_ready());
    drop(replacement);
    assert!(dropped.get());
}

#[test]
fn concurrent_dependencies_start_together_and_preserve_order() {
    let started = [Cell::new(false), Cell::new(false)];
    let ready = [Cell::new(false), Cell::new(false)];
    let wakers: [RefCell<Option<Waker>>; 2] = Default::default();
    let children = (0..2).map(|index| {
        let (started, ready, wakers) = (&started, &ready, &wakers);
        poll_fn(move |cx| {
            started[index].set(true);
            wakers[index].replace(Some(cx.waker().clone()));
            if ready[index].get() {
                Poll::Ready(Ok::<_, ()>(index))
            } else {
                Poll::Pending
            }
        })
    });
    let mut task = Task::new(task::try_join_all(children));
    assert!(task.poll().is_pending());
    assert!(started.iter().all(Cell::get));
    ready[1].set(true);
    wakers[1].borrow().as_ref().unwrap().wake_by_ref();
    assert!(task.poll().is_pending());
    ready[0].set(true);
    wakers[0].borrow().as_ref().unwrap().wake_by_ref();
    assert_eq!(task.poll(), Poll::Ready(Ok(vec![0, 1])));
}

#[test]
fn a_task_set_keeps_independent_work_alive_after_a_completion() {
    let waker = RefCell::new(None);
    let mut tasks = task::Tasks::default();
    tasks.spawn(async { 7 });
    tasks.spawn(poll_fn(|cx| {
        if waker.borrow().is_some() {
            Poll::Ready(9)
        } else {
            waker.replace(Some(cx.waker().clone()));
            Poll::Pending
        }
    }));
    assert!(tasks.is_ready());
    let (progress, completed) = tasks.poll();
    assert!(progress);
    assert_eq!(completed, vec![7]);
    assert!(!tasks.is_ready());
    assert_eq!(tasks.poll(), (false, vec![]));
    waker.borrow().as_ref().unwrap().wake_by_ref();
    assert_eq!(tasks.poll(), (true, vec![9]));
    assert!(!tasks.is_ready());
}

#[test]
fn a_nested_task_wakes_the_waiting_parent() {
    let wake = RefCell::new(None);
    let mut parent = Task::new(async {
        let mut children =
            poll_fn(|cx| Poll::Ready(task::Tasks::with_parent(cx.waker().clone()))).await;
        children.spawn(poll_fn(|cx| {
            if wake.borrow().is_some() {
                Poll::Ready(7)
            } else {
                wake.replace(Some(cx.waker().clone()));
                Poll::Pending
            }
        }));
        poll_fn(|_| {
            let (_, results) = children.poll();
            match results.into_iter().next() {
                Some(result) => Poll::Ready(result),
                None => Poll::Pending,
            }
        })
        .await
    });
    assert!(parent.poll().is_pending());
    assert!(!parent.is_ready());
    wake.borrow().as_ref().unwrap().wake_by_ref();
    assert!(parent.is_ready());
    assert_eq!(parent.poll(), Poll::Ready(7));
}
