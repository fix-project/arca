#![feature(allocator_api)]

use common::BuddyAllocator;
use kvm_ioctls::Kvm;
use kvm_ioctls::VcpuExit;

const ASM_CODE: &[u8] = include_bytes!("start.bin");

fn main() {
    use std::ptr::null_mut;

    use kvm_bindings::kvm_userspace_memory_region;
    use kvm_bindings::KVM_MEM_LOG_DIRTY_PAGES;

    // let mem_size = 0x100000000; // 4 GiB

    let mem_size = 0x100000; // 4 GiB
    let start_address = 0x8000;
    let asm_code = ASM_CODE;

    let kvm = Kvm::new().unwrap();
    let vm = kvm.create_vm().unwrap();
    let load_addr: *mut u8 = unsafe {
        libc::mmap(
            null_mut(),
            mem_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
            -1,
            0,
        ) as *mut u8
    };

    assert!(!load_addr.is_null());

    let slice = unsafe { core::slice::from_raw_parts_mut(load_addr, mem_size) };

    let allocator = &*BuddyAllocator::new(slice);

    unsafe {
        let allocation = allocator
            .reserve_boxed_slice::<u8>(start_address, asm_code.len())
            .expect("could not reserve memory for code");
        let mut allocation = allocation.assume_init();
        allocation.clone_from_slice(asm_code);
        let allocation = core::hint::black_box(allocation);
        Box::leak(allocation);
    };

    let slot = 0;
    let mem_region = kvm_userspace_memory_region {
        slot,
        guest_phys_addr: 0,
        memory_size: mem_size as u64,
        userspace_addr: load_addr as u64,
        flags: KVM_MEM_LOG_DIRTY_PAGES,
    };
    unsafe { vm.set_user_memory_region(mem_region).unwrap() };

    let mut vcpu_fd = vm.create_vcpu(0).unwrap();

    // set up the CPU in long mode
    let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();

    let mut pdpt: Box<[u64; 0x200], &BuddyAllocator> =
        unsafe { Box::new_zeroed_in(allocator).assume_init() };
    for (i, entry) in pdpt.iter_mut().enumerate() {
        *entry = ((i << 30) | 0x83) as u64;
    }

    let mut pml4: Box<[u64; 0x200], &BuddyAllocator> =
        unsafe { Box::new_zeroed_in(allocator).assume_init() };
    pml4[0] = allocator.get_offset(&*pdpt) as u64 | 3;
    Box::leak(pdpt);

    vcpu_sregs.cr4 = (1 << 5) | (1 << 7); // PAE and PGE
    vcpu_sregs.cr3 = allocator.get_offset(&*pml4) as u64;
    Box::leak(pml4);

    vcpu_sregs.idt = kvm_bindings::kvm_dtable {
        base: 0,
        limit: 0,
        padding: [0; 3],
    };
    vcpu_sregs.efer |= (1 << 8) | (1 << 10); // LME and LMA
    vcpu_sregs.cr0 |= (1 << 0) | (1 << 31); // Paging and Protection

    let code_segment = kvm_bindings::kvm_segment {
        base: 0,
        limit: 0xffffffff,
        selector: 0x8,
        type_: 0b1010,
        present: 1,
        dpl: 0,
        db: 0,
        s: 1,
        l: 1,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };
    let data_segment = kvm_bindings::kvm_segment {
        base: 0,
        limit: 0xffffffff,
        selector: 0x10,
        type_: 0b0010,
        present: 1,
        dpl: 0,
        db: 1,
        s: 1,
        l: 0,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    vcpu_sregs.cs = code_segment;
    vcpu_sregs.ds = data_segment;
    vcpu_sregs.es = data_segment;
    vcpu_sregs.fs = data_segment;
    vcpu_sregs.gs = data_segment;

    // the guest should set up its own gdt
    vcpu_sregs.gdt = kvm_bindings::kvm_dtable {
        base: 0,
        limit: 0,
        padding: [0; 3],
    };

    vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    vcpu_regs.rip = start_address as u64;
    vcpu_regs.rax = 2;
    vcpu_regs.rbx = 3;
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    loop {
        match vcpu_fd.run().expect("run failed") {
            VcpuExit::IoIn(addr, data) => {
                println!(
                    "Received an I/O in exit. Address: {:#x}. Data: {:#x}",
                    addr, data[0],
                );
            }
            VcpuExit::IoOut(addr, data) => {
                println!(
                    "Received an I/O out exit. Address: {:#x}. Data: {:#x?}",
                    addr, data,
                );
            }
            VcpuExit::MmioRead(addr, _) => {
                println!("Received an MMIO Read Request for the address {:#x}.", addr,);
            }
            VcpuExit::MmioWrite(addr, data) => {
                println!(
                    "Received an MMIO Write Request to the address {:#x}: {:x}.",
                    addr, data[0]
                );
            }
            VcpuExit::Hlt => {
                break;
            }
            r => panic!("Unexpected exit reason: {:?}", r),
        }
    }
}
