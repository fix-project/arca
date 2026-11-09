#![no_std]
#![no_main]

extern crate alloc;
extern crate user;

use alloc::vec;
use user::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let effect = Function::symbolic("bench");
    let size: Word = os::argument().try_into().unwrap();
    let limit: Word = os::argument().try_into().unwrap();
    let mut memory = vec![0u8; size.read() as usize];
    let request = Blob::new(b"bench");
    let mut epoch = 0u64;
    loop {
        for offset in (0..memory.len()).step_by(4096) {
            unsafe {
                if epoch != 0 {
                    assert_eq!(
                        memory.as_ptr().add(offset).read_volatile(),
                        (epoch - 1) as u8
                    );
                }
                memory.as_mut_ptr().add(offset).write_volatile(epoch as u8);
            }
        }
        epoch += 1;
        let reply: Word = effect
            .clone()
            .apply(request.clone())
            .call_with_current_continuation()
            .try_into()
            .unwrap();
        assert_eq!(reply.read(), epoch);
        if epoch == limit.read() {
            os::exit(Word::new(epoch));
        }
    }
}
