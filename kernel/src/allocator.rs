use crate::prelude::*;
use core::alloc::{Allocator, Layout};

use talc::{OomHandler, Span, Talc, Talck};

#[global_allocator]
static ALLOCATOR: Talck<spin::Mutex<()>, Buddy> = Talc::new(Buddy).lock();

struct Buddy;

impl OomHandler for Buddy {
    fn handle_oom(talc: &mut Talc<Self>, layout: core::alloc::Layout) -> Result<(), ()> {
        let meta = Layout::from_size_align(
            core::mem::size_of::<usize>(),
            core::mem::align_of::<usize>(),
        )
        .unwrap();
        let (meta, _) = meta.extend(meta).unwrap();
        let (layout, _) = meta.extend(layout).unwrap();
        let layout = layout.pad_to_align();
        let minimum = 1 << 21;
        let layout = if layout.size() <= minimum && layout.align() <= minimum {
            Layout::from_size_align(minimum, minimum).unwrap()
        } else {
            layout
        };
        let region = BuddyAllocator.allocate(layout).map_err(|_| ())?.as_ptr();
        unsafe {
            talc.claim(Span::from_base_size(&raw mut (*region)[0], region.len()))?;
        }
        Ok(())
    }
}
