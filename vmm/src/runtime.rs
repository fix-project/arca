use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    net::SocketAddr,
    process::ExitCode,
    slice,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{Scope, ScopedJoinHandle},
    time::{Duration, Instant},
};

use common::{
    hypercall::{self, FileInfo, TcpInfo},
    BuddyAllocator,
};
use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};
use kvm_bindings::{kvm_userspace_memory_region, CpuId, KVM_MAX_CPUID_ENTRIES};
use kvm_ioctls::{IoEventAddress, Kvm, NoDatamatch, VcpuExit, VcpuFd, VmFd};

pub use common::mmap::Mmap;
use libc::EFD_NONBLOCK;
use std::net::{TcpListener, TcpStream};
use vmm_sys_util::eventfd::EventFd;

const MEM_BASE: u64 = 0x1_0000_0000;

fn new_cpu<'scope>(
    i: usize,
    scope: &'scope Scope<'scope, '_>,
    vcpu_fd: VcpuFd,
    read_fd: EventFd,
    write_fd: EventFd,
    elf: &'scope ElfBytes<AnyEndian>,
    args: &[u64; 6],
    cpuid: &CpuId,
) -> ScopedJoinHandle<'scope, ()> {
    // set up the CPU in long mode
    let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();

    let mut pdpt: Box<[u64; 0x200], BuddyAllocator> =
        unsafe { Box::new_zeroed_in(BuddyAllocator).assume_init() };
    for (i, entry) in pdpt.iter_mut().enumerate() {
        *entry = ((i << 30) | 0x183) as u64;
    }

    let mut pml4: Box<[u64; 0x200], BuddyAllocator> =
        unsafe { Box::new_zeroed_in(BuddyAllocator).assume_init() };
    pml4[256] = (BuddyAllocator.to_offset(Box::leak(pdpt)) as u64 + MEM_BASE) | 3;
    let pml4 = Box::leak(pml4);

    vcpu_sregs.idt = kvm_bindings::kvm_dtable {
        base: 0,
        limit: 0,
        padding: [0; 3],
    };
    use common::controlreg::*;
    vcpu_sregs.cr0 |= ControlReg0::PG | ControlReg0::PE | ControlReg0::MP | ControlReg0::WP;
    vcpu_sregs.cr0 &= !ControlReg0::EM;
    vcpu_sregs.cr3 = BuddyAllocator.to_offset(pml4) as u64 + MEM_BASE;
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
        Box::<[u8; STACK_SIZE], BuddyAllocator>::new_uninit_in(BuddyAllocator).assume_init()
    };
    let stack_start = BuddyAllocator.to_offset(&*initial_stack) + MEM_BASE as usize;
    Box::leak(initial_stack);

    let pa2ka = |p: usize| p | 0xFFFF800000000000;
    vcpu_regs.rsp = pa2ka(stack_start + STACK_SIZE) as u64;
    vcpu_regs.rbp = 0;
    vcpu_regs.rdi = args[0];
    vcpu_regs.rsi = args[1];
    vcpu_regs.rdx = args[2];
    vcpu_regs.rcx = args[3];
    vcpu_regs.r8 = args[4];
    vcpu_regs.r9 = args[5];
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    // the Drop impl will wait for the child thread to exit, so we can pretend the allocator
    // lives forever
    let exit = Arc::new(AtomicBool::new(false));
    let flag = exit.clone();
    std::thread::Builder::new()
        .name(format!("Arca vCPU {i}"))
        .spawn_scoped(scope, move || {
            run_cpu(vcpu_fd, elf, flag, read_fd, write_fd);
        })
        .unwrap()
}

fn run_cpu(mut vcpu_fd: VcpuFd, elf: &ElfBytes<AnyEndian>, exit: Arc<AtomicBool>, read_fd: EventFd, write_fd: EventFd) {
    let lookup = |target| {
        let (symtab, strtab) = elf
            .symbol_table()
            .expect("could not parse ELF file")
            .expect("could not find symbol table");
        symtab.into_iter().find_map(|symbol| {
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
        })
    };

    while !exit.load(Ordering::Acquire) {
        vcpu_fd
            .set_mp_state(kvm_bindings::kvm_mp_state {
                mp_state: kvm_bindings::KVM_MP_STATE_RUNNABLE,
            })
            .unwrap();
        let vcpu_exit = vcpu_fd.run().expect("run failed");
        match vcpu_exit {
            VcpuExit::IoIn(addr, data) => match addr {
                0xe9 => {
                    io::stdin().read_exact(data).unwrap();
                }
                addr => println!(
                    "Received an I/O in exit. Address: {:#x}. Data: {:#x}",
                    addr, data[0],
                ),
            },
            VcpuExit::IoOut(addr, data) => match addr {
                0 => {
                    let mut regs = vcpu_fd.get_regs().unwrap();
                    let code = regs.rax;
                    let args = &[regs.rdi, regs.rsi, regs.rcx, regs.r10, regs.r8, regs.r9];
                    match code {
                        hypercall::EXIT => {
                            ExitCode::from(args[0] as u8).exit_process();
                        }
                        hypercall::LOG => {
                            let record: *const common::LogRecord =
                                BuddyAllocator.from_offset(args[0] as usize - MEM_BASE as usize);
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
                                    BuddyAllocator.from_offset::<u8>(target.0 - MEM_BASE as usize),
                                    target.1,
                                );
                                let file = file.map(|x| {
                                    std::str::from_raw_parts(
                                        BuddyAllocator.from_offset::<u8>(x.0 - MEM_BASE as usize),
                                        x.1,
                                    )
                                });
                                let module_path = module_path.map(|x| {
                                    std::str::from_raw_parts(
                                        BuddyAllocator.from_offset::<u8>(x.0 - MEM_BASE as usize),
                                        x.1,
                                    )
                                });
                                let message = std::str::from_raw_parts(
                                    BuddyAllocator.from_offset::<u8>(message.0 - MEM_BASE as usize),
                                    message.1,
                                );

                                log::logger().log(
                                    &log::Record::builder()
                                        .level(level)
                                        .target(target)
                                        .file(file)
                                        .line(line)
                                        .module_path(module_path)
                                        .args(format_args!("{message}"))
                                        .build(),
                                );
                            }
                        }
                        hypercall::SYMNAME => {
                            let record: *mut common::SymtabRecord =
                                BuddyAllocator.from_offset(args[0] as usize);
                            let record: &mut common::SymtabRecord = unsafe { &mut *record };
                            let target = record.addr;
                            if let Some((name, addr)) = lookup(target) {
                                record.file_len = name.len();
                                let (ptr, cap) = record.file_buffer;
                                let buffer: &mut [u8] = unsafe {
                                    core::slice::from_raw_parts_mut(
                                        BuddyAllocator.from_offset(ptr),
                                        cap,
                                    )
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
                        hypercall::MEMSET => {
                            let ptr = BuddyAllocator.from_offset(args[0] as usize);
                            let chr = args[1] as u8;
                            let len = args[2] as usize;
                            unsafe {
                                let mem = core::slice::from_raw_parts_mut(ptr, len);
                                mem.fill(chr);
                                regs.rax = mem.as_ptr() as u64;
                            }
                        }
                        hypercall::MEMCLR => {
                            let ptr = BuddyAllocator.from_offset(args[0] as usize);
                            let len = args[1] as usize;
                            unsafe {
                                let mem = core::slice::from_raw_parts_mut(ptr, len);
                                mem.fill(0);
                                regs.rax = mem.as_ptr() as u64;
                            }
                        }
                        hypercall::TCP_LISTEN => {
                            let info: &TcpInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let ip = u32::to_ne_bytes(info.ip);
                            let port = info.port;
                            let listener =
                                Box::new(TcpListener::bind(SocketAddr::from((ip, port))).unwrap());
                            let listener_ptr = Box::into_raw(listener);
                            info.id.store(listener_ptr as u64, Ordering::SeqCst);
                            info.done.store(true, Ordering::SeqCst);
                            regs.rax = listener_ptr as u64;
                        }
                        hypercall::TCP_ACCEPT => {
                            let info: &TcpInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let listener_ptr =
                                info.id.load(Ordering::SeqCst) as usize as *mut TcpListener;
                            let listener = unsafe { &*listener_ptr };
                            let (stream, _) = listener.accept().unwrap();
                            info.id
                                .store(Box::into_raw(Box::new(stream)) as u64, Ordering::SeqCst);
                            info.done.store(true, Ordering::SeqCst);
                        }
                        hypercall::TCP_SEND => {
                            let info: &TcpInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let stream_ptr =
                                info.id.load(Ordering::SeqCst) as usize as *mut TcpStream;
                            let stream = unsafe { &mut *stream_ptr };
                            let (ptr, len) = (
                                BuddyAllocator.from_offset(info.buf),
                                info.len.load(Ordering::SeqCst),
                            );
                            let slice = unsafe { slice::from_raw_parts(ptr, len) };
                            let len = stream.write(slice).unwrap_or(0);
                            // assert_eq!(len, slice.len());
                            info.len.store(len, Ordering::SeqCst);
                            info.done.store(true, Ordering::SeqCst);
                        }
                        hypercall::TCP_RECV => {
                            let info: &TcpInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let stream_ptr =
                                info.id.load(Ordering::SeqCst) as usize as *mut TcpStream;
                            let stream = unsafe { &mut *stream_ptr };
                            let (ptr, len) = (
                                BuddyAllocator.from_offset(info.buf),
                                info.len.load(Ordering::SeqCst),
                            );
                            let slice = unsafe { slice::from_raw_parts_mut(ptr, len) };
                            let len = stream.read(slice).unwrap();
                            info.len.store(len, Ordering::SeqCst);
                            info.done.store(true, Ordering::SeqCst);
                        }
                        hypercall::TCP_CLOSE => {
                            let info: &TcpInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let stream_ptr =
                                info.id.load(Ordering::SeqCst) as usize as *mut TcpStream;
                            let mut stream = *unsafe { Box::from_raw(stream_ptr) };
                            stream.flush().unwrap();
                            let _ = stream;
                            info.done.store(true, Ordering::SeqCst);
                        }
                        hypercall::FILE_OPEN => {
                            let info: &FileInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let (ptr, len) = (
                                BuddyAllocator.from_offset(info.buf),
                                info.len.load(Ordering::SeqCst),
                            );
                            let slice = unsafe { slice::from_raw_parts(ptr, len) };
                            let s = str::from_utf8(slice).unwrap();
                            let f = OpenOptions::new()
                                .read(info.read)
                                .write(info.write)
                                .create(info.create)
                                .append(info.append)
                                .truncate(info.truncate)
                                .open(s)
                                .ok()
                                .map(Box::new)
                                .map(Box::into_raw);
                            info.id
                                .store(f.unwrap_or(core::ptr::null_mut()) as u64, Ordering::SeqCst);
                            info.done.store(true, Ordering::SeqCst);
                        }
                        hypercall::FILE_READ => {
                            let info: &FileInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let file_ptr = info.id.load(Ordering::SeqCst) as *mut File;
                            let file = unsafe { &mut *file_ptr };
                            let (ptr, len) = (
                                BuddyAllocator.from_offset(info.buf),
                                info.len.load(Ordering::SeqCst),
                            );
                            let slice = unsafe { slice::from_raw_parts_mut(ptr, len) };
                            let size = file.read(slice).unwrap();
                            info.len.store(size, Ordering::SeqCst);
                            info.done.store(true, Ordering::SeqCst);
                        }
                        hypercall::FILE_WRITE => {
                            let info: &FileInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let file_ptr = info.id.load(Ordering::SeqCst) as *mut File;
                            let file = unsafe { &mut *file_ptr };
                            let (ptr, len) = (
                                BuddyAllocator.from_offset(info.buf),
                                info.len.load(Ordering::SeqCst),
                            );
                            let slice = unsafe { slice::from_raw_parts(ptr, len) };
                            let size = file.write(slice).unwrap();
                            info.len.store(size, Ordering::SeqCst);
                            info.done.store(true, Ordering::SeqCst);
                        }
                        hypercall::FILE_SEEK => {
                            let info: &FileInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let file_ptr = info.id.load(Ordering::SeqCst) as *mut File;
                            let file = unsafe { &mut *file_ptr };
                            let seek_from = match info.whence {
                                0 => SeekFrom::Start(info.offset as u64),
                                1 => SeekFrom::Current(info.offset as i64),
                                -1 => SeekFrom::End(info.offset as i64),
                                _ => panic!("invalid whence: {}", info.whence),
                            };
                            let pos = file.seek(seek_from).unwrap();
                            info.len.store(pos as usize, Ordering::SeqCst);
                            info.done.store(true, Ordering::SeqCst);
                        }
                        hypercall::FILE_CLOSE => {
                            let info: &FileInfo =
                                unsafe { &*BuddyAllocator.from_offset(args[0] as usize) };
                            let file_ptr = info.id.load(Ordering::SeqCst) as *mut File;
                            let file = *unsafe { Box::from_raw(file_ptr) };
                            let _ = file;
                        }
                        hypercall::NOTIFY_READ => {
                            read_fd.write(1).unwrap();
                        }
                        hypercall::NOTIFY_WRITE => {
                            write_fd.write(1).unwrap();
                        }
                        x => unimplemented!("hypercall {x}"),
                    };
                    vcpu_fd.set_regs(&regs).unwrap();
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
            r => {
                let error = format!("{r:?}");
                let regs = vcpu_fd.get_regs().unwrap();
                let rip = regs.rip as usize;
                if let Some((name, _)) = lookup(rip) {
                    panic!("Unexpected exit reason: {error} (in {name}) w/{regs:#x?}");
                } else {
                    panic!("Unexpected exit reason: {error} (@ {rip:#x}) w/{regs:#x?}");
                }
            }
        }
    }
}

pub struct Runtime {
    kvm: Kvm,
    vm: VmFd,
    cores: usize,
    elf: Arc<[u8]>,
}

impl Runtime {
    pub fn new(cores: usize, ram: usize, elf: Arc<[u8]>) -> Self {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        vm.create_irq_chip().unwrap();

        common::buddy::init(ram);

        let mem_region = kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: MEM_BASE,
            memory_size: BuddyAllocator.len() as u64,
            userspace_addr: BuddyAllocator.base() as u64,
            flags: 0,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        let kick = EventFd::new(EFD_NONBLOCK).unwrap();
        let call = EventFd::new(EFD_NONBLOCK).unwrap();

        let int = EventFd::new(EFD_NONBLOCK).unwrap();

        vm.register_ioevent(&kick, &IoEventAddress::Pio(0xf4), NoDatamatch)
            .unwrap();
        vm.register_irqfd(&call, 0).unwrap();
        vm.register_irqfd(&int, 1).unwrap();

        let mut last_time = None;
        ctrlc::set_handler(move || {
            let now = Instant::now();
            let diff = last_time.map(|last_time| now.duration_since(last_time));
            if let Some(diff) = diff {
                if diff < Duration::from_secs(1) {
                    log::warn!("got ^C^C; forcing immediate shutdown");
                    ExitCode::from(130).exit_process();
                }
            }
            log::info!("got ^C; sending soft shutdown");
            int.write(1).unwrap();
            last_time = Some(now);
        })
        .unwrap();

        let mut x = Self {
            kvm,
            vm,
            cores,
            elf: elf.clone(),
        };
        let elf_bytes =
            ElfBytes::<AnyEndian>::minimal_parse(&elf).expect("could not read kernel elf file");
        x.load_elf(&elf_bytes);
        x
    }

    fn load_elf(&mut self, elf: &ElfBytes<AnyEndian>) {
        let segments: Vec<ProgramHeader> = elf
            .segments()
            .expect("could not find ELF segments")
            .iter()
            .collect();

        let align_down = |alignment: usize, value: usize| value & !(alignment - 1);

        let mut reserved = HashMap::new();
        let mut reserve = |addr: usize| -> *mut () {
            let addr = align_down(4096, addr);
            if addr < MEM_BASE as usize {
                panic!("kernel wants to be loaded below {MEM_BASE:#x}");
            }
            *reserved.entry(addr).or_insert_with(|| {
                let result = BuddyAllocator.reserve_raw(addr - MEM_BASE as usize, 4096);
                assert!(!result.is_null(), "could not reserve {addr:x}");
                result
            })
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
                    let memsz = segment.p_memsz as usize;
                    let filesz = segment.p_filesz as usize;
                    let end = start + memsz;
                    let ptr = reserve_range(start, end);
                    // let offset = segment.p_offset as usize;
                    let source = &elf.segment_data(&segment).unwrap()[..filesz];
                    unsafe {
                        (&mut (*ptr))[..filesz].copy_from_slice(source);
                    }
                }
                0x60000000.. => {
                    // OS-specific (probably GNU/Linux cruft)
                }
                x => unimplemented!("segment type {x:#x}"),
            }
        }
    }

    pub fn run(&mut self, argv: Vec<String>) {
        let elf = ElfBytes::<AnyEndian>::minimal_parse(&self.elf)
            .expect("could not read kernel elf file");

        let allocator_raw = common::buddy::export();
        let allocator_raw =
            Box::into_raw_with_allocator(Box::new_in(allocator_raw, BuddyAllocator)).0;

        std::thread::scope(|s| {
            let mut cpus = vec![];

            let (p, q) = common::pipe::pipe(8192);
            let read = EventFd::new(0).unwrap();
            let write = EventFd::new(0).unwrap();
            let read_fd = read.try_clone().unwrap();
            let write_fd = write.try_clone().unwrap();
            let comm = s.spawn(move || {
                crate::comm::communication_thread(argv, crate::pipe::GuestPipe::new(read_fd, write_fd, q));
            });

            let (rx, tx) = p.into_inner();
            let rx = rx.into_inner();
            let tx = tx.into_inner();
            let (rxp, rxn) = Arc::into_raw_with_allocator(rx).0.to_raw_parts();
            let rxp = BuddyAllocator.to_offset(rxp);
            let (txp, txn) = Arc::into_raw_with_allocator(tx).0.to_raw_parts();
            let txp = BuddyAllocator.to_offset(txp);

            for i in 0..self.cores {
                let vcpu_fd = self.vm.create_vcpu(i as u64).unwrap();
                // TODO: ensure all needed features are present and disable unneeded ones
                let kvm_cpuid = self.kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();

                let mut msrs = kvm_bindings::Msrs::from_entries(&[kvm_bindings::kvm_msr_entry {
                    index: 0xC0000103,
                    ..Default::default()
                }])
                .unwrap();

                vcpu_fd.get_msrs(&mut msrs).unwrap();
                let x = &mut msrs.as_mut_slice()[0];
                x.data = i as u64;
                vcpu_fd.set_msrs(&msrs).unwrap();

                let allocator_raw_offset = BuddyAllocator.to_offset(allocator_raw);
                cpus.push(new_cpu(
                    i,
                    s,
                    vcpu_fd,
                    read.try_clone().unwrap(),
                    write.try_clone().unwrap(),
                    &elf,
                    &[
                        self.cores as u64,
                        allocator_raw_offset as u64,
                        rxp as u64,
                        rxn as u64,
                        txp as u64,
                        txn as u64,
                    ],
                    &kvm_cpuid,
                ));
            }
            for cpu in cpus {
                cpu.join().unwrap();
            }
            comm.join().unwrap();
        });
    }
}
