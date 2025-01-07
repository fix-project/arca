#![feature(allocator_api)]

use std::collections::HashSet;

use common::BuddyAllocator;
use elf::endian::AnyEndian;
use elf::segment::ProgramHeader;
use elf::ElfBytes;
use kvm_ioctls::Kvm;
use kvm_ioctls::VcpuExit;

const KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_kernel"));

fn main() {
    env_logger::init();
    use std::ptr::null_mut;

    use kvm_bindings::kvm_userspace_memory_region;
    use kvm_bindings::KVM_MEM_LOG_DIRTY_PAGES;

    let mem_size = 0x100000000; // 4 GiB

    let kernel_elf = KERNEL_ELF;

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

    let allocator = BuddyAllocator::new(slice);

    let elf =
        ElfBytes::<AnyEndian>::minimal_parse(kernel_elf).expect("could not read kernel elf file");
    let start_address = elf.ehdr.e_entry;

    let segments: Vec<ProgramHeader> = elf
        .segments()
        .expect("could not find ELF segments")
        .iter()
        .collect();

    let align_down = |alignment: u64, value: u64| value & !(alignment - 1);
    let align_up = |alignment: u64, value: u64| value + (value.wrapping_neg() & (alignment - 1));

    let mut mapped = HashSet::new();
    for segment in segments {
        match segment.p_type {
            elf::abi::SHT_NULL => {}
            elf::abi::SHT_PROGBITS => {
                let alignment = segment.p_align;
                let increment = segment.p_align as usize;
                let mut current = align_down(alignment, segment.p_paddr) as usize;
                let mut offset = align_down(alignment, segment.p_offset) as usize;
                let end = align_up(alignment, segment.p_paddr + segment.p_memsz) as usize;
                while current < end {
                    if !mapped.contains(&current) {
                        mapped.insert(current);
                        let region = allocator.reserve_raw(current & !0xFFFF800000000000, increment)
                            as *mut u8;
                        assert_ne!(region, core::ptr::null_mut());
                        let source = &kernel_elf[offset..offset + increment];
                        unsafe {
                            region.copy_from_nonoverlapping(&source[0], increment);
                        }
                    }
                    current += increment;
                    offset += increment;
                }
            }
            0x60000000.. => {
                // OS-specific (probably GNU/Linux cruft)
            }
            x => unimplemented!("segment type {x:#x}"),
        }
    }

    let mem_region = kvm_userspace_memory_region {
        slot: 0,
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
        unsafe { Box::new_zeroed_in(&allocator).assume_init() };
    for (i, entry) in pdpt.iter_mut().enumerate() {
        *entry = ((i << 30) | 0x83) as u64;
    }

    let mut pml4: Box<[u64; 0x200], &BuddyAllocator> =
        unsafe { Box::new_zeroed_in(&allocator).assume_init() };
    // pml4[0] = allocator.to_offset(&*pdpt) as u64 | 3;
    pml4[256] = allocator.to_offset(&*pdpt) as u64 | 3;
    Box::leak(pdpt);

    vcpu_sregs.cr4 = (1 << 5) | (1 << 7); // PAE and PGE
    vcpu_sregs.cr3 = allocator.to_offset(&*pml4) as u64;
    Box::leak(pml4);

    vcpu_sregs.idt = kvm_bindings::kvm_dtable {
        base: 0,
        limit: 0,
        padding: [0; 3],
    };
    vcpu_sregs.efer |= (1 << 8) | (1 << 10); // LME and LMA
    vcpu_sregs.cr0 |= (1 << 0) | (1 << 31); // Paging and Protection

    // enable SSE
    vcpu_sregs.cr0 |= 1 << 1; // Monitor Coprocessor
    vcpu_sregs.cr0 &= !(1 << 2); // x86 Emulation
    vcpu_sregs.cr4 |= (1 << 9) | (1 << 10); // SIMD

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
    vcpu_regs.rip = start_address;
    println!("starting at {:x}", start_address);
    let initial_stack = unsafe {
        Box::<[u8; 2 * 0x400 * 0x400], &BuddyAllocator>::new_uninit_in(&allocator).assume_init()
    };
    let stack_start = allocator.to_offset(&*initial_stack);
    Box::leak(initial_stack);

    let (_, data, meta) = allocator.clone().into_raw_parts();
    let pa2ka = |p: usize| (p | 0xFFFF800000000000);
    vcpu_regs.rsp = pa2ka((stack_start + (2 * 0x400 * 0x400)) as usize) as u64;
    vcpu_regs.rdi = data as u64;
    vcpu_regs.rsi = meta as u64;
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    println!("Inner is at offset {:#x}.", data);
    println!("Memory usage is {} bytes.", allocator.used_size());

    loop {
        match vcpu_fd.run().expect("run failed") {
            VcpuExit::IoIn(addr, data) => {
                println!(
                    "Received an I/O in exit. Address: {:#x}. Data: {:#x}",
                    addr, data[0],
                );
            }
            VcpuExit::IoOut(addr, data) => {
                if addr == 0xe9 {
                    let data = data[0];
                    let c = data as char;
                    print!("{c}");
                } else {
                    println!(
                        "Received an I/O out exit. Address: {:#x}. Data: {:#x?}",
                        addr, data,
                    );
                }
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
    println!("Memory usage is {} bytes.", allocator.used_size());
}
