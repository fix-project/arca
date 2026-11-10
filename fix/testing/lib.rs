#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use fixruntime::{
    Encode, EncodeView, Handle, HandleView, ObjectView, Runtime, Thunk, ThunkView, Tree,
};

fn apply(runtime: &impl Runtime, combination: Tree) -> Handle {
    let coupon = runtime
        .execute(combination)
        .expect("the reference evaluates only applications");
    let storage = runtime.storage();
    storage
        .get_tree(storage.certificate_claim(coupon).unwrap())
        .unwrap()[2]
}

fn tree(runtime: &impl Runtime, input: Tree) -> Tree {
    if input.as_handle().is_value() {
        return input;
    }
    let children: Vec<_> = runtime
        .storage()
        .get_tree(input)
        .unwrap()
        .iter()
        .map(|child| eval(runtime, *child))
        .collect();
    let result = runtime.storage().add_tree(&children);
    if input.is_tag() {
        result.tag_descriptor()
    } else {
        result
    }
}

pub fn think(runtime: &impl Runtime, thunk: Thunk) -> Handle {
    match thunk.view() {
        ThunkView::Identification(reference) => reference.into(),
        ThunkView::Selection(reference) => runtime
            .storage()
            .select(tree(runtime, reference.object_descriptor()))
            .unwrap(),
        ThunkView::Application(reference) => {
            apply(runtime, tree(runtime, reference.object_descriptor()))
        }
    }
}

fn force(runtime: &impl Runtime, mut thunk: Thunk) -> Handle {
    loop {
        let result = think(runtime, thunk);
        thunk = match result.view() {
            HandleView::Object(_) | HandleView::Ref(_) => return result,
            HandleView::Thunk(next) => *next,
            HandleView::Encode(encode) => match encode.view() {
                EncodeView::Strict(next) | EncodeView::Shallow(next) => next,
            },
        };
    }
}

fn encode(runtime: &impl Runtime, encode: Encode) -> Handle {
    let (strict, thunk) = match encode.view() {
        EncodeView::Strict(thunk) => (true, thunk),
        EncodeView::Shallow(thunk) => (false, thunk),
    };
    let result = force(runtime, thunk);
    match (strict, result.view()) {
        (true, HandleView::Ref(reference)) => reference.object_descriptor().into(),
        (false, HandleView::Object(object)) => object.into_ref().into(),
        _ => result,
    }
}

pub fn eval(runtime: &impl Runtime, input: Handle) -> Handle {
    match input.view() {
        HandleView::Ref(_) | HandleView::Thunk(_) => input,
        HandleView::Object(object) => match object.view() {
            ObjectView::Blob(_) => input,
            ObjectView::Tree(input) => tree(runtime, *input).into(),
        },
        HandleView::Encode(input) => eval(runtime, encode(runtime, *input)),
    }
}
