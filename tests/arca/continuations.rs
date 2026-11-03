#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let seed: Word = os::argument().try_into().unwrap();
    let mut bytes = [seed.read() as u8; 64];
    assert_eq!(
        unsafe { arcane::arca_call_with_current_continuation(-1) },
        -(arcane::__ERR_bad_index as i64)
    );
    let word = Word::new(7).into_inner().into_raw();
    assert_eq!(
        unsafe { arcane::arca_call_with_current_continuation(word as i64) },
        -(arcane::__ERR_bad_type as i64)
    );
    if let Ok(snapshot) = os::continuation() {
        bytes[0] += 1;
        os::exit(Tuple::from((snapshot, Blob::new(bytes))));
    }
    assert_eq!(bytes, [seed.read() as u8; 64]);
    let reply = os::argument();
    let result = Function::symbolic("captured")
        .apply(seed)
        .apply(reply)
        .call_with_current_continuation();
    os::exit(result);
}
