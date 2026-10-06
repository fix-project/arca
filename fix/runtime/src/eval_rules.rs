// right now is mostly a duplicate of evaluator but adds state tracking to see if this 
// task is finished or this has unlocked more things to do, allows it to resume
extern crate alloc;
use crate::handle::{Encode, Handle, Object, Ref, Thunk, Tree};
use crate::task_graph::Transition;
use alloc::{vec, vec::Vec};

pub trait Context {
    fn tree(&self, tree: Tree) -> Vec<Handle>;
    fn add_tree(&self, children: &[Handle]) -> Tree;
    fn execute(&self, combination: Tree) -> Handle;
    fn select(&self, selection: Tree) -> Handle;
}
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Eval(Handle),
    Force(Thunk),
    BuildTree,
    Apply,
    Select,
    ForceResult,
    AfterEncode(bool),
}
fn lift(h: Handle) -> Handle {
    match h {
        Handle::Ref(Ref::Blob(b)) => Object::Blob(b).into(),
        Handle::Ref(Ref::Tree(t)) => Object::Tree(t).into(),
        _ => h,
    }
}
fn lower(h: Handle) -> Handle {
    match h {
        Handle::Object(Object::Blob(b)) => Ref::Blob(b).into(),
        Handle::Object(Object::Tree(t)) => Ref::Tree(t).into(),
        _ => h,
    }
}
pub fn step(
    context: &impl Context,
    action: Action,
    inputs: Vec<Handle>,
) -> Transition<Action, Handle> {
    use Transition::{Complete, Continue, Fork};
    match action {
        Action::Eval(h) => match h {
            Handle::Ref(_) => Continue(Action::Eval(lift(h)), vec![]),
            Handle::Thunk(_) | Handle::Object(Object::Blob(_)) => Complete(h),
            Handle::Object(Object::Tree(t)) => Fork(
                context.tree(t).into_iter().map(Action::Eval).collect(),
                Action::BuildTree,
            ),
            Handle::Encode(e) => {
                let (t, strict) = match e {
                    Encode::Strict(t) => (t, true),
                    Encode::Shallow(t) => (t, false),
                };
                Fork(vec![Action::Force(t)], Action::AfterEncode(strict))
            }
        },
        Action::Force(t) => match t {
            Thunk::Identification(r) => Complete(lift(Handle::Ref(r))),
            Thunk::Application(t) => {
                Fork(vec![Action::Eval(Object::Tree(t).into())], Action::Apply)
            }
            Thunk::Selection(t) => Fork(vec![Action::Eval(Object::Tree(t).into())], Action::Select),
        },
        Action::BuildTree => Complete(context.add_tree(&inputs).into()),
        Action::AfterEncode(strict) => Continue(
            Action::Eval(if strict {
                lift(inputs[0])
            } else {
                lower(inputs[0])
            }),
            vec![],
        ),
        Action::Apply | Action::Select => {
            let Handle::Object(Object::Tree(t)) = inputs[0] else {
                panic!("evaluated combination must be a tree")
            };
            let h = if matches!(action, Action::Apply) {
                context.execute(t)
            } else {
                context.select(t)
            };
            Continue(Action::ForceResult, vec![h])
        }
        Action::ForceResult => match inputs[0] {
            h @ Handle::Object(_) | h @ Handle::Ref(_) => Complete(lift(h)),
            Handle::Thunk(t) | Handle::Encode(Encode::Strict(t) | Encode::Shallow(t)) => {
                Continue(Action::Force(t), vec![])
            }
        },
    }
}
