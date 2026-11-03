use std::{
    collections::HashMap,
    io::{self, Read},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
    thread::{Scope, ScopedJoinHandle},
};

use common::{
    BuddyAllocator,
    buddy::{BuddyAllocatorImpl, BuddyAllocatorRawData, MEM_BASE},
    hypercall,
};
use elf::{ElfBytes, endian::AnyEndian, segment::ProgramHeader};
use kvm_bindings::{CpuId, KVM_MAX_CPUID_ENTRIES, kvm_userspace_memory_region};
use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

pub use common::mmap::Mmap;
use libc::EFD_NONBLOCK;
use vmm_sys_util::eventfd::EventFd;

use crate::{comm::new_pipe, doorbell::HostToVMDoorBell, session::Session};

struct StopOnDrop(Arc<Session>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.stop(1);
    }
}

struct ArmedCpu(Arc<Mutex<Option<(usize, libc::pthread_t)>>>);
impl Drop for ArmedCpu {
    fn drop(&mut self) {
        *self.0.lock().unwrap() = None;
    }
}

extern "C" fn kick_signal(_: libc::c_int) {}

fn kick_signal_number() -> libc::c_int {
    static SIGNAL: OnceLock<libc::c_int> = OnceLock::new();
    *SIGNAL.get_or_init(|| unsafe {
        for signal in libc::SIGRTMIN()..=libc::SIGRTMAX() {
            let mut previous: libc::sigaction = core::mem::zeroed();
            assert_eq!(libc::sigaction(signal, core::ptr::null(), &mut previous), 0);
            if previous.sa_sigaction != libc::SIG_DFL {
                continue;
            }
            let mut action: libc::sigaction = core::mem::zeroed();
            action.sa_sigaction = kick_signal as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            assert_eq!(libc::sigaction(signal, &action, core::ptr::null_mut()), 0);
            return signal;
        }
        panic!("no unused real-time signal for KVM interruption")
    })
}

fn new_cpu<'scope>(
    i: usize,
    scope: &'scope Scope<'scope, '_>,
    vcpu_fd: VcpuFd,
    elf: &'scope ElfBytes<AnyEndian>,
    args: &[u64; 6],
    cpuid: &CpuId,
    session: Arc<Session>,
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
    vcpu_sregs.cr0 &= !(ControlReg0::EM | ControlReg0::CD | ControlReg0::NW);
    vcpu_sregs.cr3 = BuddyAllocator.to_offset(pml4) as u64 + MEM_BASE;
    vcpu_sregs.cr4 = ControlReg4::PAE
        | ControlReg4::PGE
        | ControlReg4::OSFXSR
        | ControlReg4::OSXSAVE
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

    let allocator = common::buddy::current_allocator();
    let signal = kick_signal_number();
    std::thread::Builder::new()
        .name(format!("Arca vCPU {i}"))
        .spawn_scoped(scope, move || unsafe {
            common::buddy::with_allocator(&allocator, || {
                let mut vcpu_fd = vcpu_fd;
                let immediate = &raw mut vcpu_fd.get_kvm_run().immediate_exit;
                let armed = Arc::new(Mutex::new(Some((immediate as usize, libc::pthread_self()))));
                let _armed = ArmedCpu(armed.clone());
                session.on_stop(move || {
                    if let Some((address, thread)) = *armed.lock().unwrap() {
                        (&*(address as *const AtomicU8)).store(1, Ordering::Release);
                        libc::pthread_kill(thread, signal);
                    }
                });
                let mut signals: libc::sigset_t = core::mem::zeroed();
                libc::sigemptyset(&mut signals);
                libc::sigaddset(&mut signals, signal);
                assert_eq!(
                    libc::pthread_sigmask(libc::SIG_UNBLOCK, &signals, core::ptr::null_mut()),
                    0
                );
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_cpu(&mut vcpu_fd, elf, &session)
                }))
                .is_err()
                {
                    session.stop(1);
                }
            })
        })
        .unwrap()
}

fn run_cpu(vcpu_fd: &mut VcpuFd, elf: &ElfBytes<AnyEndian>, session: &Arc<Session>) {
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

    while !session.stopped() {
        vcpu_fd
            .set_mp_state(kvm_bindings::kvm_mp_state {
                mp_state: kvm_bindings::KVM_MP_STATE_RUNNABLE,
            })
            .unwrap();
        let vcpu_exit = match vcpu_fd.run() {
            Ok(exit) => exit,
            Err(error) if error.errno() == libc::EINTR || error.errno() == libc::EAGAIN => continue,
            Err(error) => {
                log::error!("KVM_RUN failed: {error}");
                session.stop(1);
                return;
            }
        };
        match vcpu_exit {
            VcpuExit::Intr => continue,
            VcpuExit::IoIn(addr, data) => match addr {
                0xe9 => {
                    if session.captures_output() {
                        data.fill(0);
                    } else {
                        let mut input = io::stdin();
                        if session.wait(input.as_raw_fd(), libc::POLLIN).is_err() {
                            return;
                        }
                        input.read_exact(data).unwrap();
                    }
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
                            session.stop(args[0] as u8);
                            return;
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
                                BuddyAllocator.from_offset(args[0] as usize - MEM_BASE as usize);
                            let record: &mut common::SymtabRecord = unsafe { &mut *record };
                            let target = record.addr;
                            if let Some((name, addr)) = lookup(target) {
                                record.file_len = name.len();
                                let (ptr, cap) = record.file_buffer;
                                let buffer: &mut [u8] = unsafe {
                                    core::slice::from_raw_parts_mut(
                                        BuddyAllocator.from_offset(ptr - MEM_BASE as usize),
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
                            let ptr =
                                BuddyAllocator.from_offset(args[0] as usize - MEM_BASE as usize);
                            let chr = args[1] as u8;
                            let len = args[2] as usize;
                            unsafe {
                                let mem = core::slice::from_raw_parts_mut(ptr, len);
                                mem.fill(chr);
                                regs.rax = args[0];
                            }
                        }
                        hypercall::MEMCLR => {
                            let ptr =
                                BuddyAllocator.from_offset(args[0] as usize - MEM_BASE as usize);
                            let len = args[1] as usize;
                            unsafe {
                                let mem = core::slice::from_raw_parts_mut(ptr, len);
                                mem.fill(0);
                                regs.rax = args[0];
                            }
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
    vm: Arc<VmFd>,
    cores: usize,
    elf: Arc<[u8]>,
    next_pipe_idx: Arc<AtomicUsize>,
    doorbell: HostToVMDoorBell,
    interrupt: Arc<EventFd>,
    allocator: BuddyAllocatorImpl,
    guest_allocator: Option<BuddyAllocatorRawData>,
    session: Option<Arc<Session>>,
}

impl Runtime {
    /// Reserves an unused real-time signal for interrupting KVM_RUN.
    pub fn new(cores: usize, ram: usize, elf: Arc<[u8]>) -> Self {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        vm.create_irq_chip().unwrap();

        let elf_bytes =
            ElfBytes::<AnyEndian>::minimal_parse(&elf).expect("could not read kernel elf file");
        let kernel_end = elf_bytes
            .segments()
            .expect("could not find ELF segments")
            .iter()
            .filter(|segment| segment.p_type == elf::abi::PT_LOAD)
            .map(|segment| {
                let start = segment.p_paddr as usize & !0xFFFF800000000000;
                start
                    .checked_sub(MEM_BASE as usize)
                    .expect("kernel wants to be loaded below guest memory")
                    .checked_add(segment.p_memsz as usize)
                    .expect("kernel segment address overflow")
            })
            .max()
            .unwrap_or(0)
            .max(9 << 20);
        assert!(kernel_end <= ram, "kernel image exceeds guest memory");
        let allocator = BuddyAllocatorImpl::new_with_reserved(ram, kernel_end);

        let mem_region = kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: MEM_BASE,
            memory_size: allocator.len() as u64,
            userspace_addr: allocator.base() as u64,
            flags: 0,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        let interrupt = Arc::new(EventFd::new(EFD_NONBLOCK).unwrap());
        vm.register_irqfd(&interrupt, 1).unwrap();
        let _ = kick_signal_number();

        let mut x = Self {
            kvm,
            doorbell: HostToVMDoorBell::new(&vm),
            vm: Arc::new(vm),
            cores,
            elf: elf.clone(),
            next_pipe_idx: Arc::new(AtomicUsize::new(0)),
            interrupt,
            allocator,
            guest_allocator: None,
            session: None,
        };
        let allocator = x.allocator.clone();
        unsafe { common::buddy::with_allocator(&allocator, || x.load_elf(&elf_bytes)) };
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

    pub fn interrupt_handle(&self) -> Arc<EventFd> {
        self.interrupt.clone()
    }

    pub fn run(&mut self, argv: Vec<String>) -> u8 {
        let directory = std::env::current_dir().expect("current directory");
        self.run_with_io(argv, directory, false).0
    }

    pub fn run_captured(&mut self, argv: Vec<String>, directory: &Path) -> (u8, Vec<u8>) {
        self.run_with_io(argv, directory.to_owned(), true)
    }

    fn run_with_io(
        &mut self,
        argv: Vec<String>,
        directory: PathBuf,
        capture: bool,
    ) -> (u8, Vec<u8>) {
        self.run_session(argv, directory, Session::new(capture))
    }

    pub fn run_stream(
        &mut self,
        argv: Vec<String>,
        directory: &Path,
        stream: std::os::unix::net::UnixStream,
    ) -> u8 {
        self.run_session(
            argv,
            directory.to_owned(),
            Session::with_stdio(false, Some(stream)),
        )
        .0
    }

    fn run_session(
        &mut self,
        argv: Vec<String>,
        directory: PathBuf,
        session: Arc<Session>,
    ) -> (u8, Vec<u8>) {
        assert!(self.session.is_none(), "Runtime may only be run once");
        self.session = Some(session.clone());
        self.doorbell = self.doorbell.clone().with_session(session.clone());
        let allocator = self.allocator.clone();
        unsafe {
            common::buddy::with_allocator(&allocator, || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.run_inner(argv, directory, session.clone());
                }));
                if result.is_err() {
                    session.stop(1);
                }
                session.join();
            })
        };
        (session.status(), session.output())
    }

    fn run_inner(&mut self, argv: Vec<String>, directory: PathBuf, session: Arc<Session>) {
        let elf = ElfBytes::<AnyEndian>::minimal_parse(&self.elf)
            .expect("could not read kernel elf file");

        let allocator_raw = common::buddy::export();
        self.guest_allocator = Some(allocator_raw);
        let allocator_raw =
            Box::into_raw_with_allocator(Box::new_in(allocator_raw, BuddyAllocator)).0;

        std::thread::scope(|s| {
            let _stop = StopOnDrop(session.clone());
            let mut cpus = vec![];

            let (p, q) = new_pipe(&self.vm, 8192, &self.next_pipe_idx, &self.doorbell);

            let vm_cl = self.vm.clone();
            let next_pipe_cl = self.next_pipe_idx.clone();
            let doorbell = self.doorbell.clone();

            let control_session = session.clone();
            session.spawn(move || {
                crate::comm::control_thread(
                    vm_cl,
                    next_pipe_cl,
                    doorbell,
                    argv,
                    q,
                    control_session,
                    directory,
                );
            });

            let (rx, tx, _, _) = p.into_inner();
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
                    // read.try_clone().unwrap(),
                    // write.try_clone().unwrap(),
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
                    session.clone(),
                ));
            }
            for cpu in cpus {
                cpu.join().unwrap();
            }
            self.allocator.abandon();
        });
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(session) = &self.session {
            session.stop(1);
            session.join();
        }
        if let Some(allocator) = self.guest_allocator.take() {
            unsafe {
                drop(BuddyAllocatorImpl::from_raw_parts(allocator));
            }
        }
    }
}
