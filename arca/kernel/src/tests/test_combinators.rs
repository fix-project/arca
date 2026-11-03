use crate::prelude::*;
use common::elfloader;

/// Maps a partially applied curried add over a tuple, preserving each input's
/// independent continuation and evaluating the resulting applications.
#[test]
fn test_map_curried_add() {
    let add: Function = elfloader::load_elf(crate::testing::fixture("ARCA_ADD_PROGRAM")).unwrap();
    let curry: Function =
        elfloader::load_elf(crate::testing::fixture("ARCA_CURRY_PROGRAM")).unwrap();
    let map: Function = elfloader::load_elf(crate::testing::fixture("ARCA_MAP_PROGRAM")).unwrap();
    let curried_add: Function = curry
        .apply(add)
        .apply(2u64)
        .apply(7u64)
        .force()
        .try_into()
        .unwrap();
    let results: Tuple = map
        .apply(curried_add)
        .apply(Tuple::from((0u64, 1u64, 9u64, 0u64)))
        .force()
        .try_into()
        .unwrap();
    assert_eq!(results.len(), 4);
    for (result, expected) in results.into_iter().zip([7, 8, 16, 7]) {
        let application: Function = result.try_into().unwrap();
        let add: Function = application.force().try_into().unwrap();
        let sum: Word = add.force().try_into().unwrap();
        assert_eq!(sum.read(), expected);
    }
}
