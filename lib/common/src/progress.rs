use crate::protocol::control::{
    EvaluationPhase, EvaluationSnapshot, EvaluationStep, EvaluationTask, MachineProgress,
    RemoteAvailability, location_path,
};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

static DISPLAY: OnceLock<Mutex<Option<Weak<Mutex<Evaluation>>>>> = OnceLock::new();
static SUMMARIZE: AtomicBool = AtomicBool::new(false);
static ABBREVIATE: AtomicBool = AtomicBool::new(true);

pub fn abbreviate(enabled: bool) {
    ABBREVIATE.store(enabled, Ordering::Relaxed);
}

pub fn abbreviating() -> bool {
    ABBREVIATE.load(Ordering::Relaxed)
}

pub fn summarize(enabled: bool) {
    SUMMARIZE.store(enabled, Ordering::Relaxed);
}

#[derive(Default)]
struct Evaluation {
    machines: BTreeMap<String, MachineDisplay>,
    paths: BTreeMap<[u8; 16], Vec<String>>,
    closed: bool,
}

#[derive(Default)]
struct MachineDisplay {
    stats: MachineProgress,
    peak_busy: u64,
    peak_queue: u64,
    peak_memory: u64,
    completed: u64,
    privileged_completed: u64,
    cpu_ns: Option<u64>,
    bytes_sent: u64,
    bytes_received: u64,
}

impl MachineDisplay {
    fn merge(&mut self, other: Self) {
        self.stats.merge(&other.stats);
        self.peak_busy = self.peak_busy.max(other.peak_busy);
        self.peak_queue = self.peak_queue.max(other.peak_queue);
        self.peak_memory = self.peak_memory.max(other.peak_memory);
        self.completed = self.completed.max(other.completed);
        self.privileged_completed = self.privileged_completed.max(other.privileged_completed);
        self.cpu_ns = self.cpu_ns.max(other.cpu_ns);
        self.bytes_sent = self.bytes_sent.max(other.bytes_sent);
        self.bytes_received = self.bytes_received.max(other.bytes_received);
    }
}

pub fn enable() {
    DISPLAY.get_or_init(Mutex::default);
}

pub fn println(message: impl AsRef<str>) {
    line(format!("fix: {}", message.as_ref()));
}

fn line(message: impl AsRef<str>) {
    eprintln!("{}", message.as_ref());
}

pub fn write_stderr(bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut stderr = std::io::stderr().lock();
    stderr.write_all(bytes)?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        stderr.write_all(b"\n")?;
    }
    stderr.flush()
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

pub struct Progress(Option<Activity>);

struct Activity {
    label: String,
    started: Instant,
    evaluation: Option<Arc<Mutex<Evaluation>>>,
    count: Option<(Cell<u64>, bool)>,
}

impl Progress {
    pub fn activity(label: impl Into<String>) -> Self {
        if observing_evaluation() {
            return Self(None);
        }
        Self::start(label, None)
    }

    pub fn items(label: impl Into<String>, total: usize) -> Self {
        Self::counted(label, total as u64, false)
    }

    pub fn bytes(label: impl Into<String>, total: u64) -> Self {
        if observing_evaluation() {
            return Self(None);
        }
        Self::counted(label, total, true)
    }

    fn counted(label: impl Into<String>, total: u64, bytes: bool) -> Self {
        if total == 0 {
            return Self(None);
        }
        let mut progress = Self::start(label, None);
        if let Some(activity) = &mut progress.0 {
            activity.count = Some((Cell::new(0), bytes));
        }
        progress
    }

    pub fn inc(&self, amount: u64) {
        if let Some((count, _)) = self.0.as_ref().and_then(|activity| activity.count.as_ref()) {
            count.set(count.get() + amount);
        }
    }

    pub fn advised(label: impl Into<String>) -> Self {
        let Some(display) = DISPLAY.get() else {
            return Self(None);
        };
        let mut display = display.lock().unwrap();
        if display.as_ref().and_then(Weak::upgrade).is_some() {
            return Self(None);
        }
        let evaluation = Arc::new(Mutex::new(Evaluation::default()));
        *display = Some(Arc::downgrade(&evaluation));
        let label = label.into();
        Self::start(
            if label.is_empty() {
                "Evaluation".into()
            } else {
                label
            },
            Some(evaluation),
        )
    }

    fn start(label: impl Into<String>, evaluation: Option<Arc<Mutex<Evaluation>>>) -> Self {
        if DISPLAY.get().is_none() {
            return Self(None);
        }
        let label = clean(&label.into());
        line(format!("START {label}"));
        Self(Some(Activity {
            label,
            started: Instant::now(),
            evaluation,
            count: None,
        }))
    }

    pub fn finish(mut self) {
        self.close("END");
    }

    fn close(&mut self, outcome: &str) {
        let Some(activity) = self.0.take() else {
            return;
        };
        let elapsed = activity.started.elapsed().as_secs_f64();
        let detail = match activity.count {
            Some((count, true)) => transfer_detail(count.get(), elapsed),
            Some((count, false)) => format!(", {} items", count.get()),
            None => String::new(),
        };
        let evaluation = activity
            .evaluation
            .as_ref()
            .map(|state| state.lock().unwrap());
        line(format!(
            "{outcome} {} ({elapsed:.2}s{detail})",
            activity.label
        ));
        if let Some(mut evaluation) = evaluation {
            evaluation.closed = true;
            if SUMMARIZE.load(Ordering::Relaxed) {
                report_evaluation(&evaluation.machines);
            }
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.close("STOPPED");
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn advised() -> Option<Arc<Mutex<Evaluation>>> {
    DISPLAY.get()?.lock().unwrap().as_ref()?.upgrade()
}

pub fn observing_evaluation() -> bool {
    advised().is_some()
}

fn task_title(task: &EvaluationTask) -> String {
    let title = if task.title.is_empty() {
        if abbreviating() && !task.name.is_empty() {
            task.name.clone()
        } else {
            task_name(&task.id)
        }
    } else {
        task.title.clone()
    };
    let title = clean(&title);
    task.operation
        .map_or_else(|| title.clone(), |operation| format!("{operation} {title}"))
}

fn location_name(location: &[String]) -> String {
    let location = if location.is_empty() {
        "local".into()
    } else {
        location.join("!")
    };
    location
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn transfer_location(
    location: &[String],
    endpoint: &[String],
    peer: Option<[u8; 16]>,
    paths: &BTreeMap<[u8; 16], Vec<String>>,
) -> String {
    if !endpoint.is_empty()
        && let Some(path) = peer.and_then(|node| paths.get(&node))
    {
        return location_name(path);
    }
    location_name(&[location, endpoint].concat())
}

fn task_location(
    task: &crate::protocol::control::EvaluationTask,
    location: &[String],
    paths: &BTreeMap<[u8; 16], Vec<String>>,
) -> String {
    match &task.transfer {
        Some((_, destination)) => {
            transfer_location(location, destination, task.transfer_peer, paths)
        }
        None => location_name(location),
    }
}

fn reported_title(
    task: &crate::protocol::control::EvaluationTask,
    location: &[String],
    paths: &BTreeMap<[u8; 16], Vec<String>>,
) -> String {
    let title = task_title(task);
    if let Some((source, destination)) = &task.transfer {
        if source == destination {
            return title;
        }
        let source = transfer_location(location, source, task.transfer_peer, paths);
        format!("{title} (from {source})")
    } else {
        title
    }
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

fn transfer_detail(bytes: u64, elapsed: f64) -> String {
    let (scale, unit) = byte_unit(bytes);
    let amount = number(bytes as f64 / scale);
    if elapsed == 0.0 {
        return format!(", {amount} {unit}");
    }
    let rate = bytes as f64 / elapsed;
    let (scale, rate_unit) = byte_unit(rate as u64);
    format!(", {amount} {unit}, {} {rate_unit}/s", number(rate / scale))
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

fn report_evaluation(machines: &BTreeMap<String, MachineDisplay>) {
    if machines.is_empty() {
        return;
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
    for (location, machine) in machines {
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
}

pub fn evaluation_progress(snapshot: EvaluationSnapshot) {
    let Some(state) = advised() else { return };
    let mut state = state.lock().unwrap();
    if state.closed {
        return;
    }
    let mut machines = snapshot.machines;
    for stats in &mut machines {
        stats.location = location_path(stats.location.iter().map(String::as_str));
    }
    let locations: BTreeSet<_> = machines
        .iter()
        .map(|stats| location_name(&stats.location))
        .collect();
    state.machines.retain(|location, machine| {
        machine.stats.runtime.is_some() || locations.contains(location)
    });
    for stats in machines {
        if let Some(node) = stats.node {
            let preferred = state
                .paths
                .entry(node)
                .or_insert_with(|| stats.location.clone());
            if (stats.location.len(), &stats.location) < (preferred.len(), preferred) {
                *preferred = stats.location.clone();
            }
        }
        update_machine(&mut state.machines, stats);
    }
    for step in snapshot.steps {
        log_step(&state, step);
    }
}

fn update_machine(machines: &mut BTreeMap<String, MachineDisplay>, mut stats: MachineProgress) {
    let mut location = location_name(&stats.location);
    let aliases: Vec<_> = machines
        .iter()
        .filter(|(name, machine)| {
            *name == &location || (stats.node.is_some() && machine.stats.node == stats.node)
        })
        .map(|(name, _)| name.clone())
        .collect();
    if !aliases.is_empty() {
        let mut machine = machines.remove(&aliases[0]).unwrap();
        for alias in &aliases[1..] {
            machine.merge(machines.remove(alias).unwrap());
        }
        if (&machine.stats.location.len(), &machine.stats.location)
            < (&stats.location.len(), &stats.location)
        {
            stats.location = machine.stats.location.clone();
        }
        location = location_name(&stats.location);
        machines.insert(location.clone(), machine);
    }
    let machine = machines.entry(location.clone()).or_default();
    if stats.node.is_some() && stats.node == machine.stats.node {
        machine.stats.location = stats.location.clone();
    }
    if stats.node.is_some()
        && stats.node == machine.stats.node
        && stats
            .runtime
            .as_ref()
            .zip(machine.stats.runtime.as_ref())
            .is_some_and(|(now, old)| now.elapsed_ns < old.elapsed_ns)
    {
        return;
    }
    let connection = match (&stats.availability, &machine.stats.availability) {
        (
            Some(RemoteAvailability::Available { .. }),
            Some(RemoteAvailability::Available { .. }),
        ) => None,
        (Some(RemoteAvailability::Available { workers, .. }), _) => {
            Some((true, format!("{workers} workers")))
        }
        (Some(RemoteAvailability::Unavailable(error)), previous) if !matches!(previous, Some(RemoteAvailability::Unavailable(old)) if old == error) => {
            Some((
                false,
                error
                    .chars()
                    .filter(|character| !character.is_control())
                    .collect(),
            ))
        }
        _ => None,
    };
    if !stats.location.is_empty()
        && let Some((success, result)) = connection
    {
        if !matches!(
            machine.stats.availability,
            Some(RemoteAvailability::Connecting)
        ) {
            line(format!("START Connect {location}"));
        }
        line(format!(
            "{} Connect {location} ({result})",
            if success { "END" } else { "FAILED" }
        ));
    }
    if matches!(stats.availability, Some(RemoteAvailability::Connecting))
        && !matches!(
            machine.stats.availability,
            Some(RemoteAvailability::Connecting)
        )
    {
        line(format!("START Connect {location}"));
    }
    let Some(runtime) = &stats.runtime else {
        machine.stats.node = stats.node.or(machine.stats.node);
        machine.stats.location = stats.location;
        machine.stats.availability = stats.availability;
        return;
    };
    let baseline = (!stats.used && !stats.location.is_empty()).then_some(runtime);
    let previous = machine
        .stats
        .runtime
        .as_ref()
        .filter(|_| {
            machine.stats.node.is_none() || stats.node.is_none() || machine.stats.node == stats.node
        })
        .or(baseline);
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
    let used = machine.stats.used || stats.used;
    machine.stats = stats;
    machine.stats.used = used;
}

pub fn evaluation_step(step: EvaluationStep) {
    if let EvaluationPhase::Log(message) = &step.phase {
        let _ = write_stderr(message.as_bytes());
        return;
    }
    if let Some(state) = advised() {
        let state = state.lock().unwrap();
        if !state.closed {
            log_step(&state, step);
        }
    }
}

fn log_step(state: &Evaluation, step: EvaluationStep) {
    let original = location_path(step.location.iter().map(String::as_str));
    let location = if step.task.transfer.is_none() || step.task.transfer_peer.is_some() {
        step.node
            .and_then(|node| state.paths.get(&node))
            .unwrap_or(&original)
    } else {
        &original
    };
    let location_name = task_location(&step.task, location, &state.paths);
    let title = reported_title(&step.task, location, &state.paths);
    match step.phase {
        EvaluationPhase::Started => line(format!("START {location_name}: {title}")),
        EvaluationPhase::Finished {
            elapsed_ns,
            outcome,
            bytes,
        } => {
            let elapsed = elapsed_ns as f64 / 1e9;
            let detail = bytes.map_or_else(String::new, |bytes| transfer_detail(bytes, elapsed));
            line(format!(
                "{outcome} {location_name}: {title} ({elapsed:.2}s{detail})"
            ));
        }
        EvaluationPhase::Log(_) => unreachable!(),
    }
}

fn task_name(task: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut name = String::with_capacity(64);
    for byte in task {
        write!(&mut name, "{byte:02x}").unwrap();
    }
    name
}
