use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use common::{BuddyAllocator, buddy::MEM_BASE};
use kvm_ioctls::VcpuFd;

mod perf;

static OUTPUT: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

fn output_directory() -> Option<PathBuf> {
    OUTPUT.lock().unwrap().clone()
}

pub struct Capture {
    output: PathBuf,
    directory: PathBuf,
    repository: PathBuf,
    started: u64,
}

impl Capture {
    pub fn start(output: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let output = std::path::absolute(output)?;
        let repository = std::env::current_dir()?;
        if output.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "profile output exists",
            ));
        }
        let mut name = output.as_os_str().to_os_string();
        name.push(".d");
        let directory = PathBuf::from(name);
        fs::create_dir(&directory)?;
        let mut active = OUTPUT.lock().unwrap();
        if active.is_some() {
            return Err(std::io::Error::other("profiling already enabled"));
        }
        *active = Some(directory.clone());
        Ok(Self {
            output,
            directory,
            repository,
            started: clock(),
        })
    }

    fn disable(&self) {
        let mut active = OUTPUT.lock().unwrap();
        if active.as_ref() == Some(&self.directory) {
            *active = None;
        }
    }

    pub fn finish(self) -> std::io::Result<()> {
        let stopped = clock();
        self.disable();
        perf::export(&self.directory, &self.repository, &self.output)?;
        fs::write(
            self.directory.join("capture.json"),
            format!(
                "{{\"start_ns\":{},\"end_ns\":{},\"export_ns\":{}}}\n",
                self.started,
                stopped,
                clock() - stopped
            ),
        )
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.disable();
    }
}

pub(crate) fn kernel(bytes: &[u8]) {
    if let Some(directory) = output_directory() {
        let path = directory.join("kernel.elf");
        if !path.exists() {
            fs::create_dir_all(&directory).expect("create profile directory");
            fs::write(path, bytes).expect("save profile kernel");
        }
    }
}

static NEXT_PROFILE: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Profile {
    directory: PathBuf,
}

impl Profile {
    pub(crate) fn start() -> Option<Self> {
        let directory = output_directory()?.join(format!(
            "vm-{}-{}",
            std::process::id(),
            NEXT_PROFILE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).expect("create stack profile directory");
        Some(Self { directory })
    }
}

struct Probe {
    request: AtomicU64,
    phase: AtomicU8,
    stopped: AtomicBool,
    thread: libc::pthread_t,
    signal: i32,
}

struct StopProbe(Arc<Probe>);
impl Drop for StopProbe {
    fn drop(&mut self) {
        self.0.stopped.store(true, Ordering::Release);
    }
}

pub(crate) struct Recorder {
    probe: Arc<Probe>,
    output: BufWriter<File>,
    saved: usize,
    dropped: usize,
}

fn clock() -> u64 {
    let mut value: libc::timespec = unsafe { core::mem::zeroed() };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut value) },
        0
    );
    value.tv_sec as u64 * 1_000_000_000 + value.tv_nsec as u64
}

pub(crate) fn record(
    profile: Option<&Profile>,
    core: usize,
    signal: i32,
    run: impl FnOnce(Option<&mut Recorder>),
) {
    let Some(profile) = profile else {
        return run(None);
    };
    let probe = Arc::new(Probe {
        request: AtomicU64::new(0),
        phase: AtomicU8::new(0),
        stopped: AtomicBool::new(false),
        thread: unsafe { libc::pthread_self() },
        signal,
    });
    let mut output = BufWriter::new(
        File::create(profile.directory.join(format!("core-{core}-stacks.bin")))
            .expect("create stack profile"),
    );
    output
        .write_all(b"ARCASMP2")
        .expect("write stack profile header");
    let mut recorder = Recorder {
        probe: probe.clone(),
        output,
        saved: 0,
        dropped: 0,
    };
    std::thread::scope(|scope| {
        let watcher = probe.clone();
        std::thread::Builder::new()
            .name(format!("Arca profiler {core}"))
            .spawn_scoped(scope, move || {
                let mut output = BufWriter::new(
                    File::create(profile.directory.join(format!("core-{core}-pulses.bin")))
                        .expect("create profile pulse stream"),
                );
                let mut sequence = core as u64 + 1;
                while !watcher.stopped.load(Ordering::Acquire) {
                    sequence ^= sequence << 13;
                    sequence ^= sequence >> 7;
                    sequence ^= sequence << 17;
                    std::thread::sleep(Duration::from_micros(7_500 + sequence % 5_000));
                    if watcher.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let time = clock();
                    let phase = watcher.phase.load(Ordering::Acquire);
                    output
                        .write_all(&time.to_le_bytes())
                        .expect("write profile pulse");
                    output.write_all(&[phase]).expect("write profile state");
                    if phase == 1
                        && watcher
                            .request
                            .compare_exchange(0, time, Ordering::AcqRel, Ordering::Relaxed)
                            .is_ok()
                    {
                        unsafe {
                            libc::pthread_kill(watcher.thread, watcher.signal);
                        }
                    }
                }
                output.flush().expect("flush profile pulses");
            })
            .expect("start stack sampler");
        let _stop = StopProbe(probe);
        run(Some(&mut recorder));
    });
    recorder.output.flush().expect("flush stack profile");
    fs::write(
        profile.directory.join(format!("core-{core}-counts.json")),
        format!(
            "{{\"saved\":{},\"dropped\":{}}}\n",
            recorder.saved, recorder.dropped
        ),
    )
    .expect("write profile counts");
}

impl Recorder {
    pub(crate) fn before_run(&mut self, fd: &VcpuFd) {
        self.probe.phase.store(0, Ordering::Release);
        self.sample(fd);
        self.probe.phase.store(1, Ordering::Release);
    }

    pub(crate) fn after_run(&self) {
        self.probe.phase.store(0, Ordering::Release);
    }

    fn sample(&mut self, fd: &VcpuFd) {
        let requested = self.probe.request.swap(0, Ordering::AcqRel);
        if requested == 0 {
            return;
        }
        let Ok(registers) = fd.get_regs() else {
            self.dropped += 1;
            return;
        };
        let Ok(segments) = fd.get_sregs() else {
            self.dropped += 1;
            return;
        };
        let acquired = clock();
        let r = registers;
        let mut code = [0; 32];
        let mut stack = [0; 4096];
        read(segments.cr3, r.rip, &mut code);
        read(segments.cr3, r.rsp, &mut stack);
        let mut frames = [0; 48];
        frames[0] = r.rip;
        let mut depth = 1;
        {
            let mut frame = r.rbp;
            while depth < frames.len()
                && frame >= r.rsp
                && frame - r.rsp < 2 * 1024 * 1024
                && frame.is_multiple_of(8)
            {
                let mut pair = [0; 16];
                if !read(segments.cr3, frame, &mut pair) {
                    break;
                }
                let next = u64::from_le_bytes(pair[..8].try_into().unwrap());
                let ip = u64::from_le_bytes(pair[8..].try_into().unwrap());
                if ip == 0
                    || (segments.cs.dpl == 0 && ip >> 32 != 0xffff8001)
                    || (segments.cs.dpl != 0 && ip >= 0x0000_8000_0000_0000)
                {
                    break;
                }
                frames[depth] = ip;
                depth += 1;
                if next <= frame {
                    break;
                }
                frame = next;
            }
        }
        for value in [
            requested,
            acquired,
            segments.cs.dpl as u64,
            r.rip,
            r.rsp,
            r.rbp,
            depth as u64,
            r.rax,
            r.rdx,
            r.rcx,
            r.rbx,
            r.rsi,
            r.rdi,
            r.rbp,
            r.rsp,
            r.r8,
            r.r9,
            r.r10,
            r.r11,
            r.r12,
            r.r13,
            r.r14,
            r.r15,
        ] {
            self.output
                .write_all(&value.to_le_bytes())
                .expect("write profile registers");
        }
        self.output.write_all(&code).expect("write profile code");
        self.output.write_all(&stack).expect("write profile stack");
        for value in frames {
            self.output
                .write_all(&value.to_le_bytes())
                .expect("write profile frames");
        }
        self.saved += 1;
    }
}

fn physical(address: u64, output: &mut [u8]) -> bool {
    let Some(offset) = address
        .checked_sub(MEM_BASE)
        .and_then(|offset| usize::try_from(offset).ok())
    else {
        return false;
    };
    if offset > BuddyAllocator.len() || output.len() > BuddyAllocator.len() - offset {
        return false;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(
            (BuddyAllocator.base() as *const u8).add(offset),
            output.as_mut_ptr(),
            output.len(),
        );
    }
    true
}

fn word(address: u64) -> Option<u64> {
    let mut bytes = [0; 8];
    physical(address, &mut bytes).then(|| u64::from_le_bytes(bytes))
}

fn translate(cr3: u64, address: u64) -> Option<u64> {
    let mut table = cr3 & 0x000f_ffff_ffff_f000;
    for shift in [39, 30, 21, 12] {
        let entry = word(table + ((address >> shift) & 511) * 8)?;
        if entry & 1 == 0 {
            return None;
        }
        if shift == 12 || (shift < 39 && entry & 128 != 0) {
            return Some(
                (entry & 0x000f_ffff_ffff_f000 & !((1u64 << shift) - 1))
                    | (address & ((1u64 << shift) - 1)),
            );
        }
        table = entry & 0x000f_ffff_ffff_f000;
    }
    None
}

fn read(cr3: u64, mut address: u64, mut output: &mut [u8]) -> bool {
    while !output.is_empty() {
        let length = output.len().min(4096 - (address as usize & 4095));
        let Some(physical_address) = translate(cr3, address) else {
            return false;
        };
        let (first, rest) = output.split_at_mut(length);
        if !physical(physical_address, first) {
            return false;
        }
        address = address.wrapping_add(length as u64);
        output = rest;
    }
    true
}
