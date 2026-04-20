extern crate test;

use super::*;
#[test]
// Setting/clearing individual bits in u64 words to check that the bit manipulation works
fn test_bitref() {
    let mut word = 10;

    let mut r0 = BitRef::new(&mut word, 0);
    r0.set();

    let mut r1 = BitRef::new(&mut word, 1);
    r1.clear();

    let mut r2 = BitRef::new(&mut word, 2);
    r2.write(false);

    let mut r3 = BitRef::new(&mut word, 3);
    r3.write(true);

    assert_eq!(word, 9);
}

#[test]
// Setting/clearing individual bits in a BitSlice to check that the bit manipulation works
fn test_bitslice() {
    let mut words = [0; 2];
    let mut slice = BitSlice::new(128, &mut words);
    let mut r0 = slice.bit(0);
    r0.set();

    let mut r1 = slice.bit(1);
    r1.set();

    let mut r127 = slice.bit(127);
    r127.set();

    assert_eq!(words[0], 3);
    assert_eq!(
        words[127 / (core::mem::size_of::<u64>() * 8)],
        1 << (127 % (core::mem::size_of::<u64>() * 8))
    );
}

#[test]
// Basic setup + continuous element pushing to check that the allocator grows and adjusts properly
fn test_buddy_allocator() {
    let allocator = BuddyAllocatorImpl::new(0x10000000);

    let test = Box::new_in(10, allocator.clone());
    assert_eq!(*test, 10);

    let mut v = Vec::new_in(allocator.clone());
    for i in 0..10000 {
        v.push(i);
    }
}

#[test]
// Verifying that too small allocations of allocator do not panic
// Potential issue: reserve_unchecked does not validate that the requested index is within the number of blocks at that level
fn test_too_small_allocation() {
    let allocator = BuddyAllocatorImpl::new(1 << 20);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;
    let _used_before = allocator.used_size();
    let _ptr = allocator.allocate_raw(size);
}

#[test]
// Verifying allocate_raw adds size to used_size, and free_raw subtracts it back, returning usage to the original amount.
fn test_allocate_raw_and_used_size() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;
    let used_before = allocator.used_size();
    let ptr = allocator.allocate_raw(size);
    assert!(!ptr.is_null());
    assert_eq!(allocator.used_size(), used_before + size);
    allocator.free_raw(ptr, size);
    assert_eq!(allocator.used_size(), used_before);
}

#[test]
// Verifying allocate_many_raw adds size to used_size, and free_many_raw subtracts it back, returning usage to the original amount.
fn test_allocate_many_and_free_many() {
    use std::collections::BTreeSet;

    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;
    let used_before = allocator.used_size();
    let mut ptrs = [core::ptr::null_mut(); 4];
    let count = allocator.allocate_many_raw(size, &mut ptrs);
    assert_eq!(count, ptrs.len());
    assert!(ptrs.iter().all(|ptr| !ptr.is_null()));

    let unique: BTreeSet<usize> = ptrs.iter().map(|ptr| *ptr as usize).collect();
    assert_eq!(unique.len(), ptrs.len());

    allocator.free_many_raw(size, &ptrs);
    assert_eq!(allocator.used_size(), used_before);
}

#[test]
// Verifying that to_offset and from_offset roundtrip correctly
fn test_offset_roundtrip() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;
    let ptr = allocator.allocate_raw(size);
    assert!(!ptr.is_null());

    let offset = allocator.to_offset(ptr);
    let roundtrip = allocator.from_offset::<u8>(offset);
    assert_eq!(roundtrip as usize, ptr as usize);

    allocator.free_raw(ptr, size);
}

#[test]
// Verifying that reserving at zero returns a null pointer and does not add to used_size
fn test_reserve_raw_at_zero() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;
    let used_before = allocator.used_size();
    let ptr = allocator.reserve_raw(0, size);
    assert!(ptr.is_null());
    assert_eq!(allocator.used_size(), used_before);
}

#[test]
// Verifying that allocating too large returns a null pointer and does not add to used_size
fn test_allocate_raw_too_large_returns_null() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let used_before = allocator.used_size();
    let ptr = allocator.allocate_raw(allocator.total_size() * 2);
    assert!(ptr.is_null());
    assert_eq!(allocator.used_size(), used_before);
}

#[test]
// Verifying that refcnt is zero on allocate
fn test_refcnt_zero_on_allocate() {
    use core::sync::atomic::Ordering;

    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;
    let ptr = allocator.allocate_raw(size);
    assert!(!ptr.is_null());

    let refcnt = allocator.refcnt(ptr);
    assert!(!refcnt.is_null());
    let value = unsafe { (*refcnt).load(Ordering::SeqCst) };
    assert_eq!(value, 0);

    allocator.free_raw(ptr, size);
}

#[test]
// Stress testing the allocator with random allocations and frees
fn stress_test() {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let allocator = BuddyAllocatorImpl::new(0x10000000);
    allocator.set_caching(false);
    let mut v = vec![];
    let random = |limit: usize| {
        let x: u64 = RandomState::new().build_hasher().finish();
        x as usize % limit
    };
    for _ in 0..100000 {
        let used_before = allocator.used_size();
        let remaining = allocator.total_size() - used_before;
        let size = random(core::cmp::min(1 << 21, remaining / 2));
        let alloc = Box::<[u8], BuddyAllocatorImpl>::new_uninit_slice_in(size, allocator.clone());
        let used_after = allocator.used_size();
        assert!(used_after >= used_before + size);
        if !v.is_empty() && size % 3 == 0 {
            let number = random(v.len());
            for _ in 0..number {
                let index = random(v.len());
                v.remove(index);
            }
        }
        v.push(alloc);
    }
}

#[test]
// Test splitting large blocks into smaller ones
fn test_block_splitting() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let small_size = BuddyAllocatorImpl::MIN_ALLOCATION;
    let large_size = small_size * 4;

    // Allocate and free a large block
    let large_ptr = allocator.allocate_raw(large_size);
    assert!(!large_ptr.is_null());
    allocator.free_raw(large_ptr, large_size);

    // Now allocate multiple small blocks - should split the large one
    let mut small_ptrs = vec![];
    for _ in 0..4 {
        let ptr = allocator.allocate_raw(small_size);
        assert!(!ptr.is_null());
        small_ptrs.push(ptr);
    }

    // Clean up
    for ptr in small_ptrs {
        allocator.free_raw(ptr, small_size);
    }
}

#[test]
fn test_reserve_specific_addresses() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    // Get a definitely-available block
    let p = allocator.allocate_raw(size);
    assert!(!p.is_null());
    let address = allocator.to_offset(p);
    allocator.free_raw(p, size);

    // Now we should be able to reserve that exact address
    let ptr1 = allocator.reserve_raw(address, size);
    assert!(!ptr1.is_null());
    assert_eq!(allocator.to_offset(ptr1), address);

    // Reserving again should fail
    let ptr2 = allocator.reserve_raw(address, size);
    assert!(ptr2.is_null());

    allocator.free_raw(ptr1, size);
}

#[test]
// Test reserving overlapping regions
fn test_reserve_overlapping_regions() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    // Reserve a block
    let ptr1 = allocator.reserve_raw(size * 5, size);
    assert!(!ptr1.is_null());

    // Try to reserve a larger block that would overlap
    let ptr2 = allocator.reserve_raw(size * 4, size * 4);
    assert!(ptr2.is_null()); // Should fail because it overlaps with ptr1

    allocator.free_raw(ptr1, size);
}

#[test]
// Test allocating all available memory
fn test_exhaust_memory() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;
    let mut ptrs = vec![];

    // Allocate until we can't anymore
    loop {
        let ptr = allocator.allocate_raw(size);
        if ptr.is_null() {
            break;
        }
        ptrs.push(ptr);
    }

    // Verify we actually allocated something
    assert!(!ptrs.is_empty());

    // Try one more allocation - should fail
    let ptr = allocator.allocate_raw(size);
    assert!(ptr.is_null());

    // Free everything
    for ptr in ptrs {
        allocator.free_raw(ptr, size);
    }

    // Should be able to allocate again
    let ptr = allocator.allocate_raw(size);
    assert!(!ptr.is_null());
    allocator.free_raw(ptr, size);
}

#[test]
// Test mixed allocation sizes
fn test_mixed_allocation_sizes() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);

    let small = BuddyAllocatorImpl::MIN_ALLOCATION;
    let medium = small * 4;
    let large = small * 16;

    let ptr1 = allocator.allocate_raw(small);
    let ptr2 = allocator.allocate_raw(large);
    let ptr3 = allocator.allocate_raw(medium);
    let ptr4 = allocator.allocate_raw(small);

    assert!(!ptr1.is_null());
    assert!(!ptr2.is_null());
    assert!(!ptr3.is_null());
    assert!(!ptr4.is_null());

    // Verify they're all different
    let ptrs = [ptr1, ptr2, ptr3, ptr4];
    for i in 0..ptrs.len() {
        for j in (i + 1)..ptrs.len() {
            assert_ne!(ptrs[i], ptrs[j]);
        }
    }

    allocator.free_raw(ptr2, large);
    allocator.free_raw(ptr1, small);
    allocator.free_raw(ptr4, small);
    allocator.free_raw(ptr3, medium);
}

#[test]
// Test freeing in different order than allocation
fn test_free_reverse_order() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    let mut ptrs = vec![];
    for _ in 0..10 {
        let ptr = allocator.allocate_raw(size);
        assert!(!ptr.is_null());
        ptrs.push(ptr);
    }

    let used_peak = allocator.used_size();

    // Free in reverse order
    for ptr in ptrs.iter().rev() {
        allocator.free_raw(*ptr, size);
    }

    assert!(allocator.used_size() < used_peak);
}

#[test]
// Test allocation size rounding. Test after exhausting, allocator should still be usable -- no lock leak
fn allocation_rounds_up_to_pow2_and_min() {
    let a = BuddyAllocatorImpl::new(1 << 24);

    // Request sizes that aren't powers of 2
    let ptr1 = a.allocate_raw(5000); // Should round to 8192
    let ptr2 = a.allocate_raw(1000); // Should round to 4096
    let ptr3 = a.allocate_raw(10000); // Should round to 16384

    assert!(!ptr1.is_null());
    assert!(!ptr2.is_null());
    assert!(!ptr3.is_null());

    a.free_raw(ptr1, 5000);
    a.free_raw(ptr2, 1000);
    a.free_raw(ptr3, 10000);

    let used0 = a.used_size();
    let p = a.allocate_raw(5000); // rounds to 8192 (and >= 4096)
    assert!(!p.is_null());
    assert_eq!(a.used_size(), used0 + 8192);

    a.free_raw(p, 5000); // free uses same rounding path
    assert_eq!(a.used_size(), used0);
}

#[test]
// Confirm there is no overlap between levels
fn test_offset_calculation() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);

    let mut ranges = vec![];
    for level in allocator.inner.meta.level_range.clone() {
        let offset = allocator.inner.offset_of_level_words(level);
        let size = allocator.inner.size_of_level_words(level);
        ranges.push((offset, offset + size, level));
    }
    ranges.sort_by_key(|(start, _, _)| *start);

    for w in ranges.windows(2) {
        let (_s1, e1, l1) = w[0];
        let (s2, _e2, l2) = w[1];
        assert!(e1 <= s2, "overlap between level {} and level {}", l1, l2);
    }
}

#[test]
// Test bitmap boundaries
fn test_bitmap_boundaries() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);

    for level in allocator.inner.meta.level_range.clone() {
        let bits = allocator.inner.size_of_level_bits(level);
        let words = allocator.inner.size_of_level_words(level);

        // Verify words is enough to hold bits
        assert!(
            words * 64 >= bits,
            "Level {} needs {} bits but only has {} words ({} bits)",
            level,
            bits,
            words,
            words * 64
        );
    }
}

#[test]
// Test try_allocate_many_raw where everything should succeed easily
fn test_try_allocate_many_no_contention() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    let mut ptrs = [core::ptr::null_mut(); 10];
    let result = allocator.try_allocate_many_raw(size, &mut ptrs);

    assert_eq!(result, Some(10));
    assert!(ptrs.iter().all(|p| !p.is_null()));

    allocator.free_many_raw(size, &ptrs);
}

#[test]
// Testing allocating more pointers than space available, making sure bulk alloc stops cleanly when space out,
// and partial success allowed, reported successes are valid. Currently hanging
fn test_allocate_many_partial_success() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    // Request more blocks than available
    let mut ptrs = [core::ptr::null_mut(); 10000];
    let count = allocator.allocate_many_raw(size, &mut ptrs);

    // Should have allocated some but not all
    assert!(count > 0);
    assert!(count < ptrs.len());

    // All allocated pointers should be non-null
    for i in 0..count {
        assert!(!ptrs[i].is_null());
    }

    // Remaining should be null
    for i in count..ptrs.len() {
        assert!(ptrs[i].is_null());
    }

    // Clean up
    allocator.free_many_raw(size, &ptrs[0..count]);
}

#[test]
// Test that refcnt works for different allocation addresses
fn test_refcnt_different_addresses() {
    use core::sync::atomic::Ordering;

    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    let ptr1 = allocator.allocate_raw(size);
    let ptr2 = allocator.allocate_raw(size);

    let refcnt1 = allocator.refcnt(ptr1);
    let refcnt2 = allocator.refcnt(ptr2);

    // Should be different refcnt locations
    assert_ne!(refcnt1, refcnt2);

    // Both should be 0
    assert_eq!(unsafe { (*refcnt1).load(Ordering::SeqCst) }, 0);
    assert_eq!(unsafe { (*refcnt2).load(Ordering::SeqCst) }, 0);

    allocator.free_raw(ptr1, size);
    allocator.free_raw(ptr2, size);
}

#[test]
// Test null pointer refcnt
fn test_refcnt_null_pointer() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let refcnt = allocator.refcnt(core::ptr::null::<u8>());
    assert!(refcnt.is_null());
}

#[test]
// Test usage calculation
fn test_usage_calculation() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    let initial_usage = allocator.usage();

    let ptr = allocator.allocate_raw(size);
    let usage_after = allocator.usage();

    assert!(usage_after > initial_usage);
    assert!(usage_after <= 1.0);
    assert!(usage_after >= 0.0);

    allocator.free_raw(ptr, size);
}

#[test]
// Test request counting
fn test_request_counting() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    let mut before = [0; 64];
    let mut after = [0; 64];

    allocator.requests(&mut before);

    let ptr = allocator.allocate_raw(size);
    allocator.free_raw(ptr, size);

    allocator.requests(&mut after);

    // Should have incremented request count for the size level
    let level = size.next_power_of_two().ilog2() as usize;
    assert!(after[level] > before[level]);
}

#[test]
fn test_alignment_requirements() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);
    let base = allocator.base() as usize;

    for power in 12..20 {
        let size = 1 << power;
        let ptr = allocator.allocate_raw(size);
        assert!(!ptr.is_null());

        let addr = ptr as usize;
        assert_eq!(
            (addr - base) % size,
            0,
            "Allocation of size {} not aligned within arena",
            size
        );

        allocator.free_raw(ptr, size);
    }
}

#[test]
// Test clone and drop behavior
fn test_clone_and_drop() {
    let allocator = BuddyAllocatorImpl::new(1 << 24);

    let ptr1 = allocator.allocate_raw(4096);
    assert!(!ptr1.is_null());

    {
        let clone = allocator.clone();
        let ptr2 = clone.allocate_raw(4096);
        assert!(!ptr2.is_null());
        clone.free_raw(ptr2, 4096);
        // clone drops here
    }

    // Original should still work
    let ptr3 = allocator.allocate_raw(4096);
    assert!(!ptr3.is_null());

    allocator.free_raw(ptr1, 4096);
    allocator.free_raw(ptr3, 4096);
}

#[test]
// Allocate one block, compute its buddy address, and verify that reserving the buddy returns that address (if it’s free).
fn buddy_address_math_matches_reserve() {
    let a = BuddyAllocatorImpl::new(1 << 24);
    let base = a.base() as usize;

    for power in 12..18 {
        let size = 1usize << power;
        let p = a.allocate_raw(size);
        assert!(!p.is_null());

        let off = a.to_offset(p);
        let idx = off / size;
        let buddy_idx = idx ^ 1;
        let buddy_off = buddy_idx * size;

        // If the buddy is free, reserve_raw must return exactly that address.
        let b = a.reserve_raw(buddy_off, size);
        if !b.is_null() {
            assert_eq!(a.to_offset(b), buddy_off);
            a.free_raw(b, size);
        }

        a.free_raw(p, size);

        // (optional) base-relative alignment property
        assert_eq!(((p as usize) - base) % size, 0);
    }
}

#[test]
// 'Create' a known free block at a known offset by allocating a large block, then freeing it, then reserving/allocating inside it.
fn split_large_block_into_smaller_blocks() {
    let a = BuddyAllocatorImpl::new(1 << 24);

    let big = 1usize << 16; // 64KiB
    let small = 1usize << 12; // 4KiB
    let factor = big / small;

    let p = a.allocate_raw(big);
    assert!(!p.is_null());
    let off = a.to_offset(p);
    a.free_raw(p, big);

    // Now reserve all 4KiB blocks inside that 64KiB region.
    let mut blocks = Vec::new();
    for i in 0..factor {
        let q = a.reserve_raw(off + i * small, small);
        assert!(!q.is_null(), "failed to reserve sub-block {}", i);
        blocks.push(q);
    }

    // Free them back
    for q in blocks {
        a.free_raw(q, small);
    }
}

#[test]
// Reserve two buddy halves, free them, verify you can reserve the parent block at the exact parent address.
fn coalesce_two_buddies_into_parent() {
    let a = BuddyAllocatorImpl::new(1 << 24);

    let parent = 1usize << 14; // 16KiB
    let child = 1usize << 13; // 8KiB

    // Create a known free parent block at a known offset.
    let p = a.allocate_raw(parent);
    assert!(!p.is_null());
    let off = a.to_offset(p);
    a.free_raw(p, parent);

    // Reserve both children (buddies).
    let c0 = a.reserve_raw(off, child);
    let c1 = a.reserve_raw(off + child, child);
    assert!(!c0.is_null() && !c1.is_null());

    // Free both; this should coalesce into the parent.
    a.free_raw(c0, child);
    a.free_raw(c1, child);

    // Now reserving the parent at 'off' should succeed.
    let p2 = a.reserve_raw(off, parent);
    assert!(
        !p2.is_null(),
        "parent block did not reappear after coalescing"
    );
    assert_eq!(a.to_offset(p2), off);

    a.free_raw(p2, parent);
}

#[test]
// Hold one child, free the other, ensure parent reservation fails at that exact parent address.
fn no_coalesce_if_only_one_buddy_free() {
    let a = BuddyAllocatorImpl::new(1 << 24);

    let parent = 1usize << 14; // 16KiB
    let child = 1usize << 13; // 8KiB

    let p = a.allocate_raw(parent);
    assert!(!p.is_null());
    let off = a.to_offset(p);
    a.free_raw(p, parent);

    let c0 = a.reserve_raw(off, child);
    let c1 = a.reserve_raw(off + child, child);
    assert!(!c0.is_null() && !c1.is_null());

    // Free only one child
    a.free_raw(c0, child);

    // Parent must NOT be reservable while the other buddy is still held.
    let parent_try = a.reserve_raw(off, parent);
    assert!(
        parent_try.is_null(),
        "parent became available with one buddy still reserved"
    );

    // Cleanup
    a.free_raw(c1, child);

    // Now parent should be available (coalesced)
    let parent_ok = a.reserve_raw(off, parent);
    assert!(!parent_ok.is_null());
    a.free_raw(parent_ok, parent);
}

#[test]
// Free child1 then child0; ensure parent becomes available.
fn coalesce_is_order_independent() {
    let a = BuddyAllocatorImpl::new(1 << 24);
    let parent = 1usize << 15; // 32KiB
    let child = 1usize << 14; // 16KiB

    let p = a.allocate_raw(parent);
    assert!(!p.is_null());
    let off = a.to_offset(p);
    a.free_raw(p, parent);

    let c0 = a.reserve_raw(off, child);
    let c1 = a.reserve_raw(off + child, child);
    assert!(!c0.is_null() && !c1.is_null());

    a.free_raw(c1, child);
    a.free_raw(c0, child);

    let p2 = a.reserve_raw(off, parent);
    assert!(!p2.is_null());
    a.free_raw(p2, parent);
}

#[test]
// Free 4 children → coalesce to 2 parents → coalesce to 1 grandparent.
fn multi_level_coalesce_cascades() {
    let a = BuddyAllocatorImpl::new(1 << 24);

    let grand = 1usize << 15; // 32KiB
    let child = 1usize << 13; // 8KiB
    let n = grand / child; // 4

    let p = a.allocate_raw(grand);
    assert!(!p.is_null());
    let off = a.to_offset(p);
    a.free_raw(p, grand);

    let mut kids = Vec::new();
    for i in 0..n {
        let k = a.reserve_raw(off + i * child, child);
        assert!(!k.is_null());
        kids.push(k);
    }

    // Free all kids -> should coalesce up to grand
    for k in kids {
        a.free_raw(k, child);
    }

    let g = a.reserve_raw(off, grand);
    assert!(
        !g.is_null(),
        "expected full cascade coalesce to grand block"
    );
    a.free_raw(g, grand);
}

#[test]
// Size rounding edge cases: allocate_raw rounds up to power-of-two and MIN_ALLOCATION, ensuring allocations don’t fail just because size isn’t a power of two.
// Currently failing; needs to be fixed?
fn reserve_out_of_range_returns_null() {
    let a = BuddyAllocatorImpl::new(1 << 24);
    let size = 1usize << 12;

    // definitely beyond arena
    let ptr = a.reserve_raw(a.len() + size, size);
    assert!(ptr.is_null());
}

#[test]
// Testing allocate_many_raw where partial failure does not poison the lock
// Currently hanging because partial failures is not working, test after that is fixed
fn allocate_many_partial_failure_does_not_poison_lock() {
    let a = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    let mut ptrs = [core::ptr::null_mut(); 10000];
    let n = a.allocate_many_raw(size, &mut ptrs);

    assert!(n > 0);
    assert!(n < ptrs.len());

    a.free_many_raw(size, &ptrs[..n]);

    // If the lock leaked, this would hang.
    let p = a.allocate_raw(size);
    assert!(!p.is_null());
    a.free_raw(p, size);
}

#[test]
// ensures try_* returns None when lock is held.
fn try_allocate_many_returns_none_when_locked() {
    let a = BuddyAllocatorImpl::new(1 << 24);
    let size = BuddyAllocatorImpl::MIN_ALLOCATION;

    // Manually lock allocator and ensure try_* fails.
    unsafe {
        a.inner.lock();
    }
    let mut ptrs = [core::ptr::null_mut(); 4];
    let r = a.try_allocate_many_raw(size, &mut ptrs);
    assert_eq!(r, None);
    unsafe {
        a.inner.unlock();
    }

    // Now it should work
    let r2 = a.try_allocate_many_raw(size, &mut ptrs);
    assert_eq!(r2, Some(4));
    a.free_many_raw(size, &ptrs);
}

#[test]
// Interleaved patterns: A,B,C,D where (A,B) and (C,D) are buddy pairs.
// Freeing B and C alone should NOT make either parent available;
// freeing A then enables AB coalesce; freeing D then enables CD coalesce; then both parents can coalesce further.
fn interleaved_buddy_pairs_coalesce_independently_then_merge() {
    let a = BuddyAllocatorImpl::new(1 << 24);

    let grand = 1usize << 15; // 32KiB
    let parent = 1usize << 14; // 16KiB
    let child = 1usize << 13; // 8KiB

    // Known free 32KiB region
    let g = a.allocate_raw(grand);
    assert!(!g.is_null());
    let off = a.to_offset(g);
    a.free_raw(g, grand);

    // Reserve A,B,C,D as 8KiB blocks at offsets 0,1,2,3 within the 32KiB region
    let a0 = a.reserve_raw(off + 0 * child, child); // A
    let b0 = a.reserve_raw(off + 1 * child, child); // B (buddy of A)
    let c0 = a.reserve_raw(off + 2 * child, child); // C
    let d0 = a.reserve_raw(off + 3 * child, child); // D (buddy of C)
    assert!(!a0.is_null() && !b0.is_null() && !c0.is_null() && !d0.is_null());

    // Free B and C only -> neither 16KiB parent should be reservable yet.
    a.free_raw(b0, child);
    a.free_raw(c0, child);

    assert!(
        a.reserve_raw(off + 0 * parent, parent).is_null(),
        "AB parent should not exist yet"
    );
    assert!(
        a.reserve_raw(off + 1 * parent, parent).is_null(),
        "CD parent should not exist yet"
    );

    // Free A -> AB should coalesce to first 16KiB parent at off
    a.free_raw(a0, child);
    let p0 = a.reserve_raw(off + 0 * parent, parent);
    assert!(!p0.is_null(), "AB should coalesce to 16KiB");
    a.free_raw(p0, parent);

    // Free D -> CD should coalesce to second 16KiB parent at off + 16KiB
    a.free_raw(d0, child);
    let p1 = a.reserve_raw(off + 1 * parent, parent);
    assert!(!p1.is_null(), "CD should coalesce to 16KiB");
    a.free_raw(p1, parent);

    // Now both 16KiB parents are free -> should coalesce into 32KiB grandparent at off
    let g2 = a.reserve_raw(off, grand);
    assert!(
        !g2.is_null(),
        "two free 16KiB parents should coalesce to 32KiB"
    );
    a.free_raw(g2, grand);
}

#[test]
// Fragmentation scenario: partial coalescing with an obstacle.
// If one leaf remains reserved, upper levels must not fully coalesce; once obstacle freed, full coalesce should happen.
fn fragmentation_blocks_full_coalesce_until_obstacle_removed() {
    let a = BuddyAllocatorImpl::new(1 << 24);

    let big = 1usize << 16; // 64KiB region we control
    let leaf = 1usize << 12; // 4KiB
    let n = big / leaf; // 16 leaves

    // Known free 64KiB region
    let p = a.allocate_raw(big);
    assert!(!p.is_null());
    let off = a.to_offset(p);
    a.free_raw(p, big);

    // Reserve all leaves, keep one as "obstacle", free the rest.
    let mut leaves = Vec::new();
    for i in 0..n {
        let q = a.reserve_raw(off + i * leaf, leaf);
        assert!(!q.is_null());
        leaves.push(q);
    }

    let obstacle = leaves[7]; // arbitrary leaf to hold
    for (_i, q) in leaves.iter().enumerate() {
        if *q == obstacle {
            continue;
        }
        a.free_raw(*q, leaf);
    }

    // With one 4KiB still reserved, the full 64KiB block must NOT be available.
    assert!(
        a.reserve_raw(off, big).is_null(),
        "should not fully coalesce with an obstacle leaf reserved"
    );

    // Now free the obstacle leaf -> full coalesce should become possible.
    a.free_raw(obstacle, leaf);
    let big2 = a.reserve_raw(off, big);
    assert!(
        !big2.is_null(),
        "after removing obstacle, should fully coalesce back to 64KiB"
    );
    a.free_raw(big2, big);
}

#[test]
// Reserved blocks shouldn't participate in coalescing:
// if one buddy is permanently reserved (held), the parent must not become available.
fn reserved_block_prevents_coalescing() {
    let a = BuddyAllocatorImpl::new(1 << 24);

    let parent = 1usize << 14; // 16KiB
    let child = 1usize << 13; // 8KiB

    // Known free parent region
    let p = a.allocate_raw(parent);
    assert!(!p.is_null());
    let off = a.to_offset(p);
    a.free_raw(p, parent);

    // Reserve both children, but "reserve" one as a held block (simulate reservation that shouldn't coalesce).
    let held = a.reserve_raw(off, child);
    let other = a.reserve_raw(off + child, child);
    assert!(!held.is_null() && !other.is_null());

    // Free only the other -> parent must not appear
    a.free_raw(other, child);
    assert!(
        a.reserve_raw(off, parent).is_null(),
        "parent should not coalesce while one child is held/reserved"
    );

    // Once held is freed too, parent should become available
    a.free_raw(held, child);
    let p2 = a.reserve_raw(off, parent);
    assert!(!p2.is_null());
    a.free_raw(p2, parent);
}
