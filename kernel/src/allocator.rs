// use core::{
//     alloc::{Allocator, GlobalAlloc},
//     ptr::NonNull,
// };

// use crate::prelude::*;

// #[global_allocator]
// static ALLOCATOR: SystemAllocator = SystemAllocator;

// struct SystemAllocator;

// unsafe impl GlobalAlloc for SystemAllocator {
//     unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
//         let result = BuddyAllocator
//             .allocate(layout)
//             .map(|p| &raw mut (*p.as_ptr())[0])
//             .unwrap_or(core::ptr::null_mut());
//         debug_assert!(!result.is_null());
//         result
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
//         if let Some(p) = NonNull::new(ptr) {
//             BuddyAllocator.deallocate(p, layout);
//         }
//     }
// }

use crate::prelude::*;
use core::alloc::{Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;

// use talc::{OomHandler, Span, Talc, Talck};

#[global_allocator]
static ALLOCATOR: Buddy = Buddy;

struct Buddy;
unsafe impl GlobalAlloc for Buddy {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        BuddyAllocator
            .allocate(layout)
            .map(|x| &raw mut (*x.as_ptr())[0])
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(ptr) = NonNull::new(ptr) {
            BuddyAllocator.deallocate(ptr, layout);
        };
    }
}

// #[global_allocator]
// static ALLOCATOR: Talck<spin::Mutex<()>, Buddy> = Talc::new(Buddy).lock();

// struct Buddy;

// impl OomHandler for Buddy {
//     fn handle_oom(talc: &mut Talc<Self>, layout: core::alloc::Layout) -> Result<(), ()> {
//         let meta = Layout::from_size_align(
//             core::mem::size_of::<usize>(),
//             core::mem::align_of::<usize>(),
//         )
//         .unwrap();
//         let (meta, _) = meta.extend(meta).unwrap();
//         let (layout, _) = meta.extend(layout).unwrap();
//         let layout = layout.pad_to_align();
//         let minimum = 1 << 21;
//         let layout = if layout.size() <= minimum && layout.align() <= minimum {
//             Layout::from_size_align(minimum, minimum).unwrap()
//         } else {
//             layout
//         };
//         let region = BuddyAllocator.allocate(layout).map_err(|_| ())?.as_ptr();
//         unsafe {
//             talc.claim(Span::from_base_size(&raw mut (*region)[0], region.len()))?;
//         }
//         Ok(())
//     }
// }
