use crate::prelude::*;

#[test]
fn builder_clones_takes_and_calls_with_a_continuation() {
    let function = common::elfloader::load_elf(crate::testing::fixture("ARCA_BUILDER")).unwrap();
    let effect: Function = function.force().try_into().unwrap();
    let (name, mut arguments) = effect.into_inner().into_symbolic().unwrap();
    let name: Blob = name.try_into().unwrap();
    assert_eq!(&*name, b"probe");
    let argument: Word = arguments.pop_front().unwrap().try_into().unwrap();
    assert_eq!(argument.read(), 23);
    let continuation: Function = arguments.pop_front().unwrap().try_into().unwrap();
    assert!(arguments.is_empty());
    let result: Word = continuation
        .apply(Word::new(37))
        .force()
        .try_into()
        .unwrap();
    assert_eq!(result.read(), 1);
}
