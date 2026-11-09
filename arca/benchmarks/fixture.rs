#![no_std]
#![no_main]

extern crate alloc;
extern crate user;

use alloc::vec;
use user::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let args: Tuple = os::argument().try_into().unwrap();
    let mode: Word = args.get(0).try_into().unwrap();
    let first: Word = args.get(1).try_into().unwrap();
    let second: Word = args.get(2).try_into().unwrap();
    let mode = mode.read();
    if mode == 0 {
        os::exit(Word::new(first.read() + second.read()));
    }
    let mut memory = vec![0u8; first.read() as usize];
    let mut epoch = 0u64;
    let effect = Function::symbolic("bench");
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
        effect
            .clone()
            .apply(Word::new(epoch))
            .call_with_current_continuation();
    }
}
