#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

static mut ZEROES: [u8; (8 << 20) + 1] = [0; (8 << 20) + 1];
static mut INITIALIZED: [u8; 4097] = [0x29; 4097];

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let value: Word = os::argument().try_into().unwrap();
    let value = value.read() as u8;
    let initialized = (&raw mut INITIALIZED).cast::<u8>();
    unsafe {
        assert_eq!(initialized.read_volatile(), 0x29);
        assert_eq!(initialized.add(4096).read_volatile(), 0x29);
        initialized.add(4096).write_volatile(value);
    }
    let bytes = (&raw mut ZEROES).cast::<u8>();
    for offset in (0..=(8 << 20)).step_by(4096) {
        unsafe {
            assert_eq!(bytes.add(offset).read_volatile(), 0);
            bytes.add(offset).write_volatile(value);
            assert_eq!(bytes.add(offset).read_volatile(), value);
        }
    }
    os::exit(Word::new(2049));
}
