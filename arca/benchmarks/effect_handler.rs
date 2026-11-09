#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let reply = Function::symbolic("reply");
    let mut epoch = 0;
    loop {
        let request: Blob = reply
            .clone()
            .apply(Word::new(epoch))
            .call_with_current_continuation()
            .try_into()
            .unwrap();
        let mut bytes = [0; 5];
        assert_eq!(request.len(), bytes.len());
        assert_eq!(request.read(0, &mut bytes), bytes.len());
        assert_eq!(&bytes, b"bench");
        epoch += 1;
    }
}
