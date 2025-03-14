use std::{
    collections::HashMap,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
};

use common::{message::Messenger, ringbuffer, BuddyAllocator};
use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};
use kvm_bindings::{
    kvm_userspace_memory_region, CpuId, KVM_MAX_CPUID_ENTRIES, KVM_MEM_LOG_DIRTY_PAGES,
};
use kvm_ioctls::{Kvm, VcpuExit, VcpuFd};

use crate::client::Client;

pub struct Mmap {
    ptr: *mut u8,
    len: usize,
}

const KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_kernel"));

impl Mmap {
    pub fn new(len: usize) -> Self {
        let ptr: *mut u8 = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
                -1,
                0,
            ) as *mut u8
        };

        assert!(!ptr.is_null());
        Mmap { ptr, len }
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        unsafe {
            assert_eq!(libc::munmap(self.ptr as _, self.len), 0);
        }
    }
}

impl Deref for Mmap {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for Mmap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

pub struct Cpu<'a> {
    _phantom: PhantomData<&'a BuddyAllocator<'a>>,
    thread: ManuallyDrop<JoinHandle<()>>,
}

impl<'a> Cpu<'a> {
    pub fn new(
        vcpu_fd: VcpuFd,
        allocator: &'a BuddyAllocator<'a>,
        elf: Arc<ElfBytes<AnyEndian>>,
        args: &[u64; 6],
        cpuid: &CpuId,
    ) -> Self {
        // set up the CPU in long mode
        let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();

        let mut pdpt: Box<[u64; 0x200], &BuddyAllocator> =
            unsafe { Box::new_zeroed_in(allocator).assume_init() };
        for (i, entry) in pdpt.iter_mut().enumerate() {
            *entry = ((i << 30) | 0x183) as u64;
        }

        let mut pml4: Box<[u64; 0x200], &BuddyAllocator> =
            unsafe { Box::new_zeroed_in(allocator).assume_init() };
        pml4[256] = allocator.to_offset(Box::leak(pdpt)) as u64 | 3;
        let pml4 = Box::leak(pml4);

        vcpu_sregs.idt = kvm_bindings::kvm_dtable {
            base: 0,
            limit: 0,
            padding: [0; 3],
        };
        use common::controlreg::*;
        vcpu_sregs.cr0 |= ControlReg0::PG | ControlReg0::PE | ControlReg0::MP;
        vcpu_sregs.cr0 &= !ControlReg0::EM;
        vcpu_sregs.cr3 = allocator.to_offset(pml4) as u64;
        vcpu_sregs.cr4 = ControlReg4::PAE
            | ControlReg4::PGE
            | ControlReg4::OSFXSR
            | ControlReg4::OSXMMEXCPT
            | ControlReg4::FSGSBASE
            | ControlReg4::SMAP
            | ControlReg4::SMEP;
        vcpu_sregs.efer |= ExtendedFeatureEnableReg::LME
            | ExtendedFeatureEnableReg::LMA
            | ExtendedFeatureEnableReg::SCE;

        vcpu_fd.set_tsc_khz(u32::MAX / 1000).unwrap();

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

        vcpu_fd.set_cpuid2(cpuid).unwrap();

        vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

        let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
        vcpu_regs.rip = elf.ehdr.e_entry;
        const STACK_SIZE: usize = 2 * 0x400 * 0x400;
        let initial_stack = unsafe {
            Box::<[u8; STACK_SIZE], &BuddyAllocator>::new_uninit_in(allocator).assume_init()
        };
        let stack_start = allocator.to_offset(&*initial_stack);
        Box::leak(initial_stack);

        let pa2ka = |p: usize| (p | 0xFFFF800000000000);
        vcpu_regs.rsp = pa2ka(stack_start + STACK_SIZE) as u64;
        vcpu_regs.rbp = 0;
        vcpu_regs.rdi = args[0];
        vcpu_regs.rsi = args[1];
        vcpu_regs.rdx = args[2];
        vcpu_regs.rcx = args[3];
        vcpu_regs.r8 = args[4];
        vcpu_regs.r8 = args[5];
        vcpu_regs.rflags = 2;
        vcpu_fd.set_regs(&vcpu_regs).unwrap();

        // the Drop impl will wait for the child thread to exit, so we can pretend the allocator
        // lives forever
        let elf: Arc<ElfBytes<'static, AnyEndian>> = unsafe { core::mem::transmute(elf) };
        let exit = Arc::new(AtomicBool::new(false));
        let flag = exit.clone();
        let a: BuddyAllocator<'static> = unsafe { core::mem::transmute(allocator.clone()) };
        let thread = std::thread::spawn(move || {
            Self::run(vcpu_fd, a, elf, flag);
        });
        Cpu {
            _phantom: PhantomData,
            thread: ManuallyDrop::new(thread),
        }
    }

    fn run(
        mut vcpu_fd: VcpuFd,
        allocator: BuddyAllocator<'a>,
        elf: Arc<ElfBytes<AnyEndian>>,
        exit: Arc<AtomicBool>,
    ) {
        let mut log_record: usize = 0;
        let mut symtab_record: usize = 0;
        while !exit.load(Ordering::Acquire) {
            vcpu_fd
                .set_mp_state(kvm_bindings::kvm_mp_state {
                    mp_state: kvm_bindings::KVM_MP_STATE_RUNNABLE,
                })
                .unwrap();
            match vcpu_fd.run().expect("run failed") {
                VcpuExit::IoIn(addr, data) => println!(
                    "Received an I/O in exit. Address: {:#x}. Data: {:#x}",
                    addr, data[0],
                ),
                VcpuExit::IoOut(addr, data) => match addr {
                    0 => {
                        if data[0] == 0 {
                            return;
                        }
                        ExitCode::from(data[0]).exit_process();
                    }
                    1 => {
                        log_record = u32::from_ne_bytes(data.try_into().unwrap()) as usize;
                    }
                    2 => {
                        log_record |= (u32::from_ne_bytes(data.try_into().unwrap()) as usize) << 32;
                        let record: *const common::LogRecord = allocator.from_offset(log_record);
                        unsafe {
                            let common::LogRecord {
                                level,
                                target,
                                file,
                                line,
                                module_path,
                                message,
                            } = *record;
                            let level = log::Level::iter().nth(level as usize - 1).unwrap();
                            let target = std::str::from_raw_parts(
                                allocator.from_offset::<u8>(target.0),
                                target.1,
                            );
                            let file = file.map(|x| {
                                std::str::from_raw_parts(allocator.from_offset::<u8>(x.0), x.1)
                            });
                            let module_path = module_path.map(|x| {
                                std::str::from_raw_parts(allocator.from_offset::<u8>(x.0), x.1)
                            });
                            let message = std::str::from_raw_parts(
                                allocator.from_offset::<u8>(message.0),
                                message.1,
                            );

                            log::logger().log(
                                &log::Record::builder()
                                    .level(level)
                                    .target(target)
                                    .file(file)
                                    .line(line)
                                    .module_path(module_path)
                                    .args(format_args!("{}", message))
                                    .build(),
                            );
                        }
                    }
                    3 => {
                        symtab_record = u32::from_ne_bytes(data.try_into().unwrap()) as usize;
                    }
                    4 => {
                        symtab_record |=
                            (u32::from_ne_bytes(data.try_into().unwrap()) as usize) << 32;
                        let record: *mut common::SymtabRecord =
                            allocator.from_offset(symtab_record);
                        let record: &mut common::SymtabRecord = unsafe { &mut *record };
                        let (symtab, strtab) = elf
                            .symbol_table()
                            .expect("could not parse ELF file")
                            .expect("could not find symbol table");
                        let target = record.addr;
                        if let Some((name, addr)) = symtab.into_iter().find_map(|symbol| {
                            let index = symbol.st_name as usize;
                            let addr = symbol.st_value as usize;
                            let size = symbol.st_size as usize;
                            let end = addr + size;
                            if target >= addr && target < end {
                                let name = strtab.get(index).unwrap();
                                let name = rustc_demangle::demangle(name);
                                Some((name.to_string(), addr))
                            } else {
                                None
                            }
                        }) {
                            record.file_len = name.len();
                            let (ptr, cap) = record.file_buffer;
                            let buffer: &mut [u8] = unsafe {
                                core::slice::from_raw_parts_mut(allocator.from_offset(ptr), cap)
                            };
                            if name.len() > cap {
                                buffer.copy_from_slice(&name.as_bytes()[..cap]);
                            } else {
                                buffer[..name.len()].copy_from_slice(name.as_bytes());
                            }
                            record.offset = record.addr - addr;
                            record.addr = addr;
                            record.found = true;
                        } else {
                            record.found = false;
                        }
                    }
                    0xe9 => {
                        let data = data[0];
                        let c = data as char;
                        eprint!("{c}");
                    }
                    _ => {
                        println!(
                            "Received an I/O out exit. Address: {:#x}. Data: {:#x}",
                            addr, data[0],
                        );
                    }
                },
                VcpuExit::MmioRead(addr, _) => {
                    println!("Received an MMIO Read Request for the address {:#x}.", addr,);
                }
                VcpuExit::MmioWrite(addr, data) => {
                    println!(
                        "Received an MMIO Write Request to the address {:#x}: {:x}.",
                        addr, data[0]
                    );
                }
                r => panic!("Unexpected exit reason: {:?}", r),
            }
        }
    }
}

impl Drop for Cpu<'_> {
    fn drop(&mut self) {
        // TODO: uncomment this to force an exit as soon as possible
        // self.exit.store(true, Ordering::Release);
        unsafe {
            let thread = ManuallyDrop::take(&mut self.thread);
            thread.join().unwrap();
        }
    }
}

pub struct Runtime<'a> {
    client: Client<'a>,
    _cpus: Vec<Cpu<'a>>,
    _allocator: Box<BuddyAllocator<'a>>,
}

impl<'a> Runtime<'a> {
    pub fn new<'kvm: 'a, 'ram: 'a>(kvm: &'kvm Kvm, ram: &'ram mut [u8], cores: usize) -> Self {
        let vm = kvm.create_vm().unwrap();
        vm.create_irq_chip().unwrap();

        let allocator = Box::new(BuddyAllocator::new(ram));

        let elf = ElfBytes::<AnyEndian>::minimal_parse(KERNEL_ELF)
            .expect("could not read kernel elf file");

        let segments: Vec<ProgramHeader> = elf
            .segments()
            .expect("could not find ELF segments")
            .iter()
            .collect();

        let align_down = |alignment: usize, value: usize| value & !(alignment - 1);

        let mut reserved = HashMap::new();
        let mut reserve = |addr: usize| -> *mut () {
            let addr = align_down(4096, addr);
            *reserved
                .entry(addr)
                .or_insert_with(|| allocator.reserve_raw(addr, 4096))
        };
        let mut reserve_range = |start: usize, end: usize| -> *mut [u8] {
            let mut current = (start / 4096) * 4096;
            let len = end - start;
            let result = reserve(current).wrapping_byte_add(start - current);
            current += 4096;
            while current < end {
                reserve(current);
                current += 4096;
            }
            core::ptr::slice_from_raw_parts_mut(result as *mut u8, len)
        };

        // let mut mapped = HashSet::new();
        for segment in segments {
            match segment.p_type {
                elf::abi::SHT_NULL => {}
                elf::abi::SHT_PROGBITS => {
                    let start = segment.p_paddr as usize & !0xFFFF800000000000;
                    let sz = segment.p_memsz as usize;
                    let end = start + sz;
                    let ptr = reserve_range(start, end);
                    let offset = segment.p_offset as usize;
                    let source = &KERNEL_ELF[offset..offset + sz];
                    unsafe {
                        (*ptr).copy_from_slice(source);
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
            memory_size: allocator.len() as u64,
            userspace_addr: allocator.base() as u64,
            flags: KVM_MEM_LOG_DIRTY_PAGES,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        let allocator_raw = allocator.clone().into_raw_parts();
        let allocator_raw = Box::into_raw(Box::new_in(allocator_raw, &*allocator));

        // This is okay because the allocator is in a fixed location on the heap, and will not be
        // freed until after everything holding this reference has been destroyed.
        let a: &'a BuddyAllocator<'a> = unsafe { core::mem::transmute(&*allocator) };
        let (endpoint1, endpoint2) = ringbuffer::pair(1024, a);
        let endpoint_raw = Box::into_raw(Box::new_in(
            endpoint2.into_raw_parts(&allocator),
            &*allocator,
        ));

        let messenger = Mutex::new(Messenger::new(endpoint1));
        let client = Client::new(messenger);

        let elf = Arc::new(elf);

        let mut cpus = vec![];
        for i in 0..cores {
            let vcpu_fd = vm.create_vcpu(i as u64).unwrap();
            // TODO: ensure all needed features are present and disable unneeded ones
            let kvm_cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();

            let mut msrs = kvm_bindings::Msrs::from_entries(&[kvm_bindings::kvm_msr_entry {
                index: 0xC0000103,
                ..Default::default()
            }])
            .unwrap();

            vcpu_fd.get_msrs(&mut msrs).unwrap();
            let x = &mut msrs.as_mut_slice()[0];
            x.data = i as u64;
            vcpu_fd.set_msrs(&msrs).unwrap();

            cpus.push(Cpu::new(
                vcpu_fd,
                a,
                elf.clone(),
                &[
                    allocator.to_offset(allocator_raw) as u64,
                    allocator.to_offset(endpoint_raw) as u64,
                    0,
                    0,
                    0,
                    0,
                ],
                &kvm_cpuid,
            ));
        }
        Self {
            client,
            _cpus: cpus,
            _allocator: allocator,
        }
    }

    pub fn client(&self) -> &Client<'a> {
        &self.client
    }
}
