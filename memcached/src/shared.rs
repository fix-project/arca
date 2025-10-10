use core::{marker::PhantomData, mem::MaybeUninit};

use user::io::Monitor;
use user::prelude::*;

pub auto trait Shareable {}

impl<T> !Shareable for &T {}
impl<T> !Shareable for &mut T {}
impl<T> !Shareable for *const T {}
impl<T> !Shareable for *mut T {}

pub struct Shared<T: Shareable> {
    monitor: Monitor,
    _phantom: PhantomData<T>,
}

impl<T: Shareable> Shared<T> {
    pub fn new(value: T) -> Self {
        let size = core::mem::size_of_val(&value);
        let mut page = Page::new(size);
        page.with_mut(|slice| {
            #[allow(clippy::transmute_ptr_to_ref)]
            let r: &mut MaybeUninit<T> = unsafe { core::mem::transmute(slice.as_ptr()) };
            r.write(value);
        });
        let monitor = Monitor::new();
        monitor.set(page);
        Shared {
            monitor,
            _phantom: PhantomData,
        }
    }

    pub fn inspect(&self, f: impl FnOnce(&T) -> Value) -> Value {
        self.monitor.enter(|x| {
            let page: Page = x.get().try_into().unwrap();
            page.with_ref(|slice| {
                #[allow(clippy::transmute_ptr_to_ref)]
                let r: &T = unsafe { core::mem::transmute(slice.as_ptr()) };
                f(r)
            })
        })
    }

    pub fn modify(&self, f: impl FnOnce(&mut T) -> Value) -> Value {
        self.monitor.enter(|x| {
            let mut page: Page = x.get().try_into().unwrap();
            let result = page.with_mut(|slice| {
                #[allow(clippy::transmute_ptr_to_ref)]
                let r: &mut T = unsafe { core::mem::transmute(slice.as_ptr()) };
                f(r)
            });
            x.set(page);
            result
        })
    }
}
