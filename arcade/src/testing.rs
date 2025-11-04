use kernel::prelude::*;
extern crate alloc;

fn test_serialization_round_trip() {
    let arca = Arca::new();
    let func = Function::from(arca);
    let val = Value::Function(func);
    let bytes_vec = postcard::to_allocvec(&val).unwrap();
    log::info!("{}", bytes_vec.len());
    log::info!("{:?}", bytes_vec);
    let new: Value = postcard::from_bytes(&bytes_vec).unwrap();
    log::info!("{:?}", new);
    assert_eq!(new, val);
}

pub fn test_runner() {
    test_serialization_round_trip();
}
