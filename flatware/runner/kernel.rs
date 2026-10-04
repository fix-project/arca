#![no_std]
#![no_main]
use fix::{Evaluator, Handle, HandleView, ObjectView, Storage, Tree, arca::FixOnArca};
use flatware_protocol::Object;
use kernel::{
    host::{fs::File, os},
    prelude::*,
};

fn import(object: Object, storage: &dyn Storage) -> Handle {
    match object {
        Object::Blob(bytes) => storage.add_blob(&bytes).into(),
        Object::Tree(children) => {
            let children = children
                .into_iter()
                .map(|child| import(child, storage))
                .collect::<Vec<_>>();
            storage.add_tree(&children).into()
        }
    }
}

fn export(handle: Handle, storage: &dyn Storage) -> Object {
    let object = match handle.view() {
        HandleView::Object(object) => *object,
        HandleView::Ref(reference) => reference.object_descriptor(),
        _ => panic!("expected a materialized Flatware result"),
    };
    match object.view() {
        ObjectView::Blob(blob) => Object::Blob(storage.get_blob(*blob).unwrap().into()),
        ObjectView::Tree(tree) => Object::Tree(
            storage
                .get_tree(*tree)
                .unwrap()
                .iter()
                .map(|&child| export(child, storage))
                .collect(),
        ),
    }
}

#[kmain]
fn main() {
    let args = os::argv();
    assert_eq!(args.len(), 3);
    let input = fix::preprocessor::read_file(&args[1]).expect("read Flatware input");
    let input: Object = postcard::from_bytes(&input).expect("decode Flatware input");
    let evaluator = Evaluator::new(FixOnArca::default());
    let combination =
        Tree::try_from(import(input, evaluator.storage())).expect("Flatware combination");
    let result = evaluator.eval(combination.into_ref().application().strict().into());
    let result = postcard::to_allocvec(&export(result, evaluator.storage()))
        .expect("encode Flatware result");
    let mut output =
        File::open(&args[2], false, true, true, false, true).expect("open Flatware output");
    assert_eq!(output.write_exact(&result), result.len());
    drop(output);
    kernel::shutdown();
}
