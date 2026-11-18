use crate::protocol::control::{
    EvaluationOperation, EvaluationSnapshot, EvaluationStep, MachineProgress, RemoteAvailability,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    collections::BTreeMap,
    io::IsTerminal,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

type TaskKey = ([u8; 32], Option<EvaluationOperation>);

static DISPLAY: OnceLock<Display> = OnceLock::new();
static SUMMARIZE: AtomicBool = AtomicBool::new(false);

pub fn summarize(enabled: bool) {
    SUMMARIZE.store(enabled, Ordering::Relaxed);
}

struct Display {
    bars: MultiProgress,
    active: Mutex<Vec<Weak<State>>>,
    terminal: bool,
    advised: Mutex<Option<Weak<State>>>,
}

struct State {
    bar: ProgressBar,
    detail: Mutex<String>,
    started: Instant,
    last_report: AtomicU64,
    advice: Mutex<BTreeMap<TaskKey, Task>>,
    parents: Mutex<BTreeMap<TaskKey, TaskKey>>,
    labels: Mutex<BTreeMap<[u8; 32], String>>,
    input: Mutex<Option<[u8; 32]>>,
    machines: Mutex<BTreeMap<String, MachineDisplay>>,
    reported: AtomicBool,
    steps: AtomicU64,
    closed: Mutex<bool>,
}

struct MachineDisplay {
    progress: Progress,
    tasks: BTreeMap<TaskKey, Task>,
    stats: MachineProgress,
    peak_busy: u64,
    peak_queue: u64,
    peak_memory: u64,
    completed: u64,
    privileged_completed: u64,
    cpu_ns: Option<u64>,
    cpu_load: Option<f64>,
    bytes_sent: u64,
    bytes_received: u64,
}

struct Task {
    progress: Progress,
    title: String,
    operation: Option<EvaluationOperation>,
    depth: usize,
}

pub fn enable() {
    DISPLAY.get_or_init(|| {
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(Duration::from_millis(200));
                let Some(display) = DISPLAY.get() else {
                    continue;
                };
                let active = {
                    let mut active = display.active.lock().unwrap();
                    active.retain(|state| state.strong_count() > 0);
                    active.iter().filter_map(Weak::upgrade).collect::<Vec<_>>()
                };
                for state in active {
                    let closed = state.closed.lock().unwrap();
                    if *closed || state.bar.is_finished() {
                        continue;
                    }
                    let message = state.detail.lock().unwrap().clone();
                    state.bar.set_message(message.clone());
                    state.bar.tick();
                    let elapsed = state.started.elapsed().as_secs();
                    let interval = elapsed / 15 + 1;
                    if !display.terminal
                        && elapsed >= 1
                        && interval > state.last_report.load(Ordering::Relaxed)
                    {
                        state.last_report.store(interval, Ordering::Relaxed);
                        let count = state.bar.length().map_or_else(String::new, |total| {
                            format!(" ({}/{total})", state.bar.position())
                        });
                        println(format!(
                            "{}: {message}{count} [{elapsed}s elapsed]",
                            state.bar.prefix()
                        ));
                    }
                }
            }
        });
        Display {
            bars: MultiProgress::new(),
            active: Mutex::new(Vec::new()),
            terminal: std::io::stderr().is_terminal(),
            advised: Mutex::new(None),
        }
    });
}

pub fn println(message: impl AsRef<str>) {
    line(format!("fix: {}", message.as_ref()));
}

fn line(message: impl AsRef<str>) {
    if let Some(display) = DISPLAY.get().filter(|display| display.terminal) {
        let _ = display.bars.println(message.as_ref());
    } else {
        eprintln!("{}", message.as_ref());
    }
}

pub fn suspend<R>(write: impl FnOnce() -> R) -> R {
    match DISPLAY.get() {
        Some(display) => display.bars.suspend(write),
        None => write(),
    }
}

pub fn write_stderr(bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let write = || {
        let mut stderr = std::io::stderr().lock();
        stderr.write_all(bytes)?;
        if !bytes.is_empty() && !bytes.ends_with(b"\n") {
            stderr.write_all(b"\n")?;
        }
        stderr.flush()
    };
    suspend(write)
}

pub fn forward_stderr(reader: impl std::io::Read + Send + 'static) {
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(reader);
        let mut line = Vec::new();
        while reader
            .read_until(b'\n', &mut line)
            .is_ok_and(|length| length > 0)
        {
            if write_stderr(&line).is_err() {
                break;
            }
            line.clear();
        }
    });
}

#[derive(Default)]
pub struct Stderr {
    pending: Vec<u8>,
}

impl std::io::Write for Stderr {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        if let Some(end) = self.pending.iter().rposition(|byte| *byte == b'\n') {
            write_stderr(&self.pending[..=end])?;
            self.pending.drain(..=end);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.pending.is_empty() {
            write_stderr(&self.pending)?;
            self.pending.clear();
        }
        Ok(())
    }
}

impl Drop for Stderr {
    fn drop(&mut self) {
        use std::io::Write;
        let _ = self.flush();
    }
}

pub struct Progress(Option<Arc<State>>);

fn style(total: Option<u64>, bytes: bool) -> ProgressStyle {
    let template = match (total, bytes) {
        (Some(total), true) if total > 1 => {
            "{spinner} {prefix} [{elapsed_precise}] [{wide_bar}] {bytes}/{total_bytes} {msg}"
        }
        (Some(total), false) if total > 1 => {
            "{spinner} {prefix} [{elapsed_precise}] [{wide_bar}] {pos}/{len} {msg}"
        }
        (Some(_), true) => "{spinner} {prefix} [{elapsed_precise}] {bytes}/{total_bytes} {msg}",
        (Some(_), false) => "{spinner} {prefix} [{elapsed_precise}] {pos}/{len} {msg}",
        _ => "[{elapsed_precise}] {prefix}: {wide_msg}",
    };
    ProgressStyle::with_template(template).unwrap()
}

impl Progress {
    pub fn activity(label: impl Into<String>) -> Self {
        if advised().is_some() {
            return Self(None);
        }
        Self::new(label.into(), None, false)
    }

    pub fn advised(label: impl Into<String>) -> Self {
        if advised().is_some() {
            return Self(None);
        }
        let progress = Self::new(label.into(), None, false);
        if let (Some(display), Some(state)) = (DISPLAY.get(), &progress.0) {
            *display.advised.lock().unwrap() = Some(Arc::downgrade(state));
        }
        progress
    }

    pub fn items(label: impl Into<String>, total: usize) -> Self {
        Self::new(label.into(), Some(total as u64), false)
    }

    pub fn bytes(label: impl Into<String>, total: u64) -> Self {
        if observing_evaluation() {
            return Self(None);
        }
        Self::new(label.into(), Some(total), true)
    }

    fn new(label: String, total: Option<u64>, bytes: bool) -> Self {
        Self::new_after(label, total, bytes, None)
    }

    fn new_after(
        label: String,
        total: Option<u64>,
        bytes: bool,
        parent: Option<&ProgressBar>,
    ) -> Self {
        let Some(display) = DISPLAY.get() else {
            return Self(None);
        };
        if total == Some(0) {
            return Self(None);
        }
        let label: String = label
            .chars()
            .filter(|character| !character.is_control())
            .collect();
        let bar = ProgressBar::hidden();
        bar.set_style(style(total, bytes));
        if let Some(total) = total {
            bar.set_length(total);
        }
        bar.set_prefix(label);
        let bar = match parent {
            Some(parent) => display.bars.insert_after(parent, bar),
            None => display.bars.add(bar),
        };
        let state = Arc::new(State {
            bar,
            detail: Mutex::new(String::new()),
            started: Instant::now(),
            last_report: AtomicU64::new(0),
            advice: Mutex::new(BTreeMap::new()),
            parents: Mutex::new(BTreeMap::new()),
            labels: Mutex::new(BTreeMap::new()),
            input: Mutex::new(None),
            machines: Mutex::new(BTreeMap::new()),
            reported: AtomicBool::new(false),
            steps: AtomicU64::new(0),
            closed: Mutex::new(false),
        });
        display.active.lock().unwrap().push(Arc::downgrade(&state));

        Self(Some(state))
    }

    pub fn evaluation_names(
        &self,
        input: Option<[u8; 32]>,
        labels: impl IntoIterator<Item = ([u8; 32], String)>,
    ) {
        if let Some(state) = advised().or_else(|| self.0.clone()) {
            *state.input.lock().unwrap() = input;
            let mut names = state.labels.lock().unwrap();
            for (name, label) in labels {
                names.entry(data_name(name)).or_insert(label);
            }
        }
    }

    pub fn message(&self, message: impl Into<String>) {
        if let Some(state) = &self.0 {
            *state.detail.lock().unwrap() = message.into();
        }
    }

    pub fn inc(&self, amount: u64) {
        if let Some(state) = &self.0 {
            state.bar.inc(amount);
        }
    }

    pub fn position(&self, completed: u64, total: u64) {
        if let Some(state) = &self.0 {
            position(state, completed, total);
        }
    }

    pub fn finish(self) {
        self.close("completed");
    }

    fn close(&self, outcome: &str) {
        let Some(state) = &self.0 else { return };
        let mut closed = state.closed.lock().unwrap();
        if *closed {
            return;
        }
        *closed = true;
        state.bar.finish_and_clear();
        report_evaluation(state, outcome);
        if let Some(display) = DISPLAY.get() {
            display.bars.remove(&state.bar);
        }
    }
}

fn position(state: &State, completed: u64, total: u64) {
    if total == 0 || completed > total {
        return;
    }
    if completed == total {
        state.bar.unset_length();
        state.bar.set_style(style(None, false));
        *state.detail.lock().unwrap() =
            format!("{completed}/{total} reported units complete; task still running");
    } else {
        state.bar.set_style(style(Some(total), false));
        state.bar.set_length(total);
        state.bar.set_position(completed);
    }
}

fn data_name(mut name: [u8; 32]) -> [u8; 32] {
    name[31] &= 0xe0;
    name
}

fn task_title(
    id: [u8; 32],
    title: &str,
    operation: Option<EvaluationOperation>,
    labels: &BTreeMap<[u8; 32], String>,
) -> String {
    let label = labels.get(&data_name(id));
    let title = match (label, title.is_empty()) {
        (Some(label), false) if label != title => format!("{label}: {title}"),
        (Some(label), _) => label.clone(),
        (None, false) => title.into(),
        (None, true) => task_name(&id),
    };
    let title = operation.map_or_else(|| title.clone(), |operation| format!("{operation} {title}"));
    title
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn location_name(location: &[String]) -> String {
    let location = if location.is_empty() {
        "local".into()
    } else {
        location.join(" / ")
    };
    location
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn reported_title(
    task: &crate::protocol::control::EvaluationTask,
    machine: &MachineProgress,
    labels: &BTreeMap<[u8; 32], String>,
) -> String {
    let title = task_title(task.id, &task.title, task.operation, labels);
    if let Some((source, destination)) = &task.transfer {
        let source = [machine.location.as_slice(), source.as_slice()].concat();
        let destination = [machine.location.as_slice(), destination.as_slice()].concat();
        format!(
            "{title} ({} -> {})",
            location_name(&source),
            location_name(&destination)
        )
    } else {
        title
    }
}

pub fn observing_evaluation() -> bool {
    advised().is_some()
}

fn availability(status: Option<&RemoteAvailability>, used: bool) -> String {
    let message = match status {
        Some(RemoteAvailability::Configured) => "configured; not contacted".into(),
        Some(RemoteAvailability::Connecting) => "connecting".into(),
        Some(RemoteAvailability::Available { workers, .. }) => {
            if *workers == 0 {
                "storage available".into()
            } else if used {
                "available".into()
            } else {
                "available; unused".into()
            }
        }
        Some(RemoteAvailability::Unavailable(error)) => format!("unavailable: {error}"),
        None => String::new(),
    };
    message
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn byte_unit(bytes: u64) -> (f64, &'static str) {
    for (scale, unit) in [
        (1u64 << 40, "TiB"),
        (1 << 30, "GiB"),
        (1 << 20, "MiB"),
        (1 << 10, "KiB"),
    ] {
        if bytes >= scale {
            return (scale as f64, unit);
        }
    }
    (1.0, "B")
}

fn number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    let precision = if value >= 1.0 {
        1
    } else {
        (1.0 - value.log10().floor()) as usize
    };
    format!("{value:.precision$}")
}

fn capacity(machine: &MachineProgress) -> (Option<u64>, Option<u64>) {
    if let Some(runtime) = &machine.runtime {
        (Some(runtime.workers), Some(runtime.memory_total))
    } else if let Some(RemoteAvailability::Available {
        workers,
        memory_bytes,
    }) = &machine.availability
    {
        (Some(*workers), *memory_bytes)
    } else {
        (None, None)
    }
}

fn load_bar(fraction: f64) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * 10.0).round() as usize;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(10 - filled))
}

impl MachineDisplay {
    fn new(location: &str) -> Self {
        let progress = Progress::new(format!("  {location}"), None, false);
        if let Some(state) = &progress.0 {
            state.last_report.store(u64::MAX, Ordering::Relaxed);
        }
        Self {
            progress,
            tasks: BTreeMap::new(),
            stats: MachineProgress::default(),
            peak_busy: 0,
            peak_queue: 0,
            peak_memory: 0,
            completed: 0,
            privileged_completed: 0,
            cpu_ns: None,
            cpu_load: None,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}

fn report_evaluation(state: &State, outcome: &str) -> bool {
    let machines = state.machines.lock().unwrap();
    if machines.is_empty() || state.reported.swap(true, Ordering::Relaxed) {
        return false;
    }
    for task in state.advice.lock().unwrap().values() {
        if let Some(progress) = &task.progress.0 {
            progress.bar.finish_and_clear();
        }
    }
    for machine in machines.values() {
        if let Some(progress) = &machine.progress.0 {
            progress.bar.finish_and_clear();
        }
        for task in machine.tasks.values() {
            if let Some(progress) = &task.progress.0 {
                progress.bar.finish_and_clear();
            }
        }
    }
    line(format!(
        "{} {outcome} ({:.2}s)",
        state.bar.prefix(),
        state.started.elapsed().as_secs_f64()
    ));
    if !SUMMARIZE.load(Ordering::Relaxed) {
        return true;
    }
    line("");
    let (memory_scale, memory_unit) = byte_unit(
        machines
            .values()
            .map(|machine| {
                capacity(&machine.stats)
                    .1
                    .unwrap_or(0)
                    .max(machine.peak_memory)
            })
            .max()
            .unwrap_or(0),
    );
    let (sent_scale, sent_unit) = byte_unit(
        machines
            .values()
            .map(|machine| machine.bytes_sent)
            .max()
            .unwrap_or(0),
    );
    let (received_scale, received_unit) = byte_unit(
        machines
            .values()
            .map(|machine| machine.bytes_received)
            .max()
            .unwrap_or(0),
    );
    let (cpu_scale, cpu_unit) = if machines
        .values()
        .filter_map(|machine| machine.cpu_ns)
        .max()
        .unwrap_or(0)
        < 1_000_000_000
    {
        (1e6, "ms")
    } else {
        (1e9, "s")
    };
    line(format!(
        "{:<16} {:>5} {:>9} {:>9} {:>7} {:>5} {:>14} {:>12} {:>14}",
        "Node",
        "Apps",
        "Collector",
        format!("CPU ({cpu_unit})"),
        "Workers",
        "Qmax",
        format!("Mem peak ({memory_unit})"),
        format!("Sent ({sent_unit})"),
        format!("Received ({received_unit})")
    ));
    for (location, machine) in machines.iter() {
        let measured = machine.stats.runtime.is_some();
        let (workers, memory) = capacity(&machine.stats);
        let peak_workers = if measured {
            machine.peak_busy.to_string()
        } else {
            "—".into()
        };
        let peak_memory = if measured {
            number(machine.peak_memory as f64 / memory_scale)
        } else {
            "—".into()
        };
        let values = [
            if measured {
                machine.completed.to_string()
            } else {
                "—".into()
            },
            if measured {
                machine.privileged_completed.to_string()
            } else {
                "—".into()
            },
            machine
                .cpu_ns
                .map_or_else(|| "—".into(), |ns| number(ns as f64 / cpu_scale)),
            workers
                .filter(|workers| *workers > 0)
                .map_or(peak_workers.clone(), |total| {
                    format!("{peak_workers}/{total}")
                }),
            if measured {
                machine.peak_queue.to_string()
            } else {
                "—".into()
            },
            memory
                .filter(|memory| *memory > 0)
                .map_or(peak_memory.clone(), |total| {
                    format!("{peak_memory}/{}", number(total as f64 / memory_scale))
                }),
            if measured {
                number(machine.bytes_sent as f64 / sent_scale)
            } else {
                "—".into()
            },
            if measured {
                number(machine.bytes_received as f64 / received_scale)
            } else {
                "—".into()
            },
        ];
        line(format!(
            "{location:<16} {:>5} {:>9} {:>9} {:>7} {:>5} {:>14} {:>12} {:>14}",
            values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7]
        ));
    }
    true
}

pub fn evaluation_progress(snapshot: EvaluationSnapshot) {
    let Some(state) = advised() else { return };
    let closed = state.closed.lock().unwrap();
    if *closed {
        return;
    }
    for stats in snapshot.machines {
        let location = location_name(&stats.location);
        let mut labels = if stats.location.is_empty() {
            state.labels.lock().unwrap().clone()
        } else {
            BTreeMap::new()
        };
        for (name, label) in &stats.labels {
            labels
                .entry(data_name(*name))
                .or_insert_with(|| label.clone());
        }
        if stats.location.is_empty() {
            *state.labels.lock().unwrap() = labels.clone();
        }
        if stats.location.is_empty() {
            let mut parents = state.parents.lock().unwrap();
            for task in &stats.tasks {
                for child in &task.children {
                    parents.insert(*child, (task.id, task.operation));
                }
            }
        }
        let mut machines = state.machines.lock().unwrap();
        let machine = machines
            .entry(location.clone())
            .or_insert_with(|| MachineDisplay::new(&location));
        if !stats.location.is_empty()
            && matches!(stats.availability, Some(RemoteAvailability::Available { workers, .. }) if workers > 0)
            && !matches!(
                machine.stats.availability,
                Some(RemoteAvailability::Available { .. })
            )
            && let Some(RemoteAvailability::Available { workers, .. }) = &stats.availability
        {
            line(format!("{location}: available ({workers} workers)"));
        }
        let Some(runtime) = &stats.runtime else {
            machine.stats.location = stats.location;
            machine.stats.availability = stats.availability;
            machine.progress.message(availability(
                machine.stats.availability.as_ref(),
                machine.stats.used,
            ));
            continue;
        };
        let baseline = (!stats.used && !stats.location.is_empty()).then_some(runtime);
        let previous = machine.stats.runtime.as_ref().or(baseline);
        let delta = |now: u64, old: u64| now.checked_sub(old).unwrap_or(now);
        machine.completed += delta(runtime.completed, previous.map_or(0, |old| old.completed));
        machine.privileged_completed += delta(
            runtime.privileged_completed,
            previous.map_or(0, |old| old.privileged_completed),
        );
        machine.bytes_sent += delta(runtime.bytes_sent, previous.map_or(0, |old| old.bytes_sent));
        machine.bytes_received += delta(
            runtime.bytes_received,
            previous.map_or(0, |old| old.bytes_received),
        );
        if let Some(cpu) = runtime.cpu_ns
            && previous.is_none_or(|old| old.elapsed_ns != runtime.elapsed_ns)
        {
            machine.cpu_ns = Some(
                machine.cpu_ns.unwrap_or(0)
                    + delta(cpu, previous.and_then(|old| old.cpu_ns).unwrap_or(0)),
            );
        }
        machine.peak_busy = machine.peak_busy.max(runtime.peak_busy_workers);
        machine.peak_queue = machine.peak_queue.max(runtime.peak_queued);
        machine.peak_memory = machine.peak_memory.max(runtime.peak_memory);
        let cpu = previous.and_then(|old| {
            let elapsed = runtime.elapsed_ns.checked_sub(old.elapsed_ns)?;
            let cpu = runtime.cpu_ns?.checked_sub(old.cpu_ns?)?;
            (elapsed > 0).then_some(cpu as f64 / elapsed as f64 / runtime.workers.max(1) as f64)
        });
        if let Some(load) = cpu {
            machine.cpu_load = Some(load);
        } else if runtime.cpu_ns.is_none()
            || previous.is_some_and(|old| old.elapsed_ns > runtime.elapsed_ns)
        {
            machine.cpu_load = None;
        }
        let cpu = machine.cpu_load.map_or_else(
            || "CPU —".into(),
            |load| format!("CPU {} {:.0}%", load_bar(load), load * 100.0),
        );
        let memory = if runtime.memory_total > 0 {
            let (scale, unit) = byte_unit(runtime.memory_total.max(runtime.memory_used));
            format!(
                "Mem {} {}/{} {unit}",
                load_bar(runtime.memory_used as f64 / runtime.memory_total as f64),
                number(runtime.memory_used as f64 / scale),
                number(runtime.memory_total as f64 / scale)
            )
        } else {
            "Mem —".into()
        };
        let unused = if !stats.location.is_empty() && !machine.stats.used && !stats.used {
            "  unused"
        } else {
            ""
        };
        machine.progress.message(format!("{cpu}  {memory}{unused}"));
        {
            let parents: BTreeMap<_, _> = stats
                .tasks
                .iter()
                .flat_map(|task| {
                    task.children
                        .iter()
                        .map(move |child| (*child, (task.id, task.operation)))
                })
                .collect();
            let mut active = std::collections::BTreeSet::new();
            for entry in &stats.tasks {
                let named = labels.contains_key(&data_name(entry.id));
                let root = state
                    .input
                    .lock()
                    .unwrap()
                    .is_some_and(|input| data_name(input) == data_name(entry.id));
                if stats.location.is_empty() && entry.operation != Some(EvaluationOperation::Load) {
                    continue;
                }
                if entry.title.is_empty() && entry.progress.is_some() && !named && !root {
                    continue;
                }
                let key = (entry.id, entry.operation);
                active.insert(key);
                let mut depth = 1;
                let mut ancestor = key;
                let mut visited = std::collections::BTreeSet::new();
                while visited.insert(ancestor) {
                    let Some(parent) = parents.get(&ancestor) else {
                        break;
                    };
                    depth += 1;
                    ancestor = *parent;
                }
                let task = machine.tasks.entry(key).or_insert_with(|| Task {
                    progress: Progress(None),
                    title: entry.title.clone(),
                    operation: entry.operation,
                    depth,
                });
                if let Some(bar) = &task.progress.0 {
                    bar.bar.set_prefix(format!(
                        "{}{}: {}",
                        "  ".repeat(depth),
                        location,
                        reported_title(entry, &stats, &labels)
                    ));
                }
                task.title = entry.title.clone();
                if let Some((completed, total)) = entry.progress {
                    if task.progress.0.is_none() && total > 0 && completed < total {
                        task.progress = Progress::new(
                            format!("{}: {}", location, reported_title(entry, &stats, &labels)),
                            Some(total),
                            false,
                        );
                    }
                    task.progress.position(completed, total);
                }
            }
            machine.tasks.retain(|id, _| active.contains(id));
        }
        let used = machine.stats.used || stats.used;
        machine.stats = stats;
        machine.stats.used = used;
    }
    let steps = snapshot.steps;
    let machines = state.machines.lock().unwrap();
    let busy: u64 = machines
        .values()
        .filter_map(|machine| machine.stats.runtime.as_ref())
        .map(|runtime| runtime.busy_workers)
        .sum();
    let queued: u64 = machines
        .values()
        .filter_map(|machine| machine.stats.runtime.as_ref())
        .map(|runtime| runtime.queued)
        .sum();
    let completed: u64 = machines.values().map(|machine| machine.completed).sum();
    let labels = state.labels.lock().unwrap();
    let local = state.advice.lock().unwrap();
    let label_names = &*labels;
    let current = local
        .iter()
        .map(|(key, task)| ("local", key, task, label_names))
        .chain(machines.iter().flat_map(|(location, machine)| {
            machine
                .tasks
                .iter()
                .map(move |(key, task)| (location.as_str(), key, task, label_names))
        }))
        .filter(|(_, _, task, _)| !task.title.is_empty())
        .min_by_key(|(_, _, task, _)| {
            if task.operation == Some(EvaluationOperation::Apply) {
                0
            } else {
                1
            }
        })
        .map(|(location, key, task, labels)| {
            format!(
                " | {location}: {}",
                task_title(key.0, &task.title, task.operation, labels)
            )
        })
        .unwrap_or_default();
    *state.detail.lock().unwrap() =
        format!("{busy} busy, {queued} queued, {completed} applications{current}");
    drop(local);
    drop(labels);
    drop(machines);
    drop(closed);
    for step in steps {
        evaluation_step(step);
    }
}

pub fn evaluation_step(step: EvaluationStep) {
    let Some(state) = advised() else { return };
    let closed = state.closed.lock().unwrap();
    if *closed {
        return;
    }
    let location = location_name(&step.location);
    let key = (step.task.id, step.task.operation);
    let mut labels = state.labels.lock().unwrap().clone();
    if step.location.is_empty() {
        state.advice.lock().unwrap().remove(&key);
    } else if let Some(machine) = state.machines.lock().unwrap().get_mut(&location) {
        for (name, label) in &machine.stats.labels {
            labels.insert(data_name(*name), label.clone());
        }
        machine.tasks.remove(&key);
    }
    let title = task_title(step.task.id, &step.task.title, step.task.operation, &labels);
    let completed = state.steps.fetch_add(1, Ordering::Relaxed) + 1;
    line(format!(
        "[{completed}] {}{location}: {title} ({:.2}s)",
        if step.success { "" } else { "FAILED " },
        step.elapsed_ns as f64 / 1e9
    ));
}

fn task_name(task: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut name = String::with_capacity(64);
    for byte in task {
        write!(&mut name, "{byte:02x}").unwrap();
    }
    name
}

fn advised() -> Option<Arc<State>> {
    let state = DISPLAY.get()?.advised.lock().unwrap().as_ref()?.upgrade()?;
    (!state.bar.is_finished()).then_some(state)
}

fn task<'a>(state: &State, tasks: &'a mut BTreeMap<TaskKey, Task>, id: TaskKey) -> &'a mut Task {
    let mut depth = 1;
    let parents = state.parents.lock().unwrap();
    let mut ancestor = id;
    let mut visited = std::collections::BTreeSet::new();
    while visited.insert(ancestor) {
        let Some(parent) = parents.get(&ancestor) else {
            break;
        };
        depth += 1;
        ancestor = *parent;
    }
    drop(parents);
    tasks.entry(id).or_insert_with(|| Task {
        progress: Progress(None),
        title: String::new(),
        operation: id.1,
        depth,
    })
}

fn counted_task<'a>(
    state: &State,
    tasks: &'a mut BTreeMap<TaskKey, Task>,
    id: TaskKey,
) -> &'a mut Task {
    let parent = state.parents.lock().unwrap().get(&id).copied();
    let parent_bar = parent
        .and_then(|id| tasks.get(&id))
        .and_then(|task| task.progress.0.as_ref())
        .map(|state| state.bar.clone())
        .unwrap_or_else(|| state.bar.clone());
    let task = task(state, tasks, id);
    if task.progress.0.is_none() {
        let title = task_title(id.0, &task.title, id.1, &state.labels.lock().unwrap());
        task.progress = Progress::new_after(
            format!("{}local: {title}", "  ".repeat(task.depth)),
            None,
            false,
            Some(&parent_bar),
        );
    }
    task
}

pub fn advice(id: [u8; 32], operation: Option<EvaluationOperation>, key: &[u8], payload: &[u8]) {
    let Some(state) = advised() else { return };
    let closed = state.closed.lock().unwrap();
    if *closed {
        return;
    }
    let mut tasks = state.advice.lock().unwrap();
    match key {
        b"fix::title" => {
            let Ok(title) = std::str::from_utf8(payload) else {
                return;
            };
            let task = task(&state, &mut tasks, (id, operation));
            task.title = title
                .chars()
                .filter(|character| !character.is_control())
                .collect();

            let title = task_title(
                id,
                &task.title,
                task.operation,
                &state.labels.lock().unwrap(),
            );
            if let Some(progress) = &task.progress.0 {
                progress
                    .bar
                    .set_prefix(format!("{}local: {title}", "  ".repeat(task.depth)));
            }
        }
        b"fix::progress" if payload.len() == 16 => {
            let completed = u64::from_le_bytes(payload[..8].try_into().unwrap());
            let total = u64::from_le_bytes(payload[8..].try_into().unwrap());
            if completed > total {
                return;
            }
            let task = counted_task(&state, &mut tasks, (id, operation));
            if let Some(state) = &task.progress.0 {
                state.last_report.store(0, Ordering::Relaxed);
            }
            task.progress.position(completed, total);
        }
        _ => (),
    }
}

pub fn tree_progress(id: [u8; 32], children: &[[u8; 32]], completed: u64, total: u64) {
    let Some(state) = advised() else { return };
    let closed = state.closed.lock().unwrap();
    if *closed {
        return;
    }
    for child in children {
        state
            .parents
            .lock()
            .unwrap()
            .entry((*child, Some(EvaluationOperation::Evaluate)))
            .or_insert((id, Some(EvaluationOperation::Evaluate)));
    }
    let named = state.labels.lock().unwrap().contains_key(&data_name(id));
    let root = state
        .input
        .lock()
        .unwrap()
        .is_some_and(|input| data_name(input) == data_name(id));
    if root {
        position(&state, completed, total);
        return;
    }
    if !named {
        return;
    }
    let mut tasks = state.advice.lock().unwrap();
    {
        let task = counted_task(
            &state,
            &mut tasks,
            (id, Some(EvaluationOperation::Evaluate)),
        );
        task.progress.message("evaluating Tree children");
        task.progress.position(completed, total);
    }
}

pub fn task_finished(id: [u8; 32], operation: Option<EvaluationOperation>) {
    if let Some(state) = advised() {
        let mut tasks = state.advice.lock().unwrap();
        tasks.remove(&(id, operation));
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.close("stopped");
    }
}
