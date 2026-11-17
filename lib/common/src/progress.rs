use crate::protocol::control::{EvaluationOperation, EvaluationSnapshot, MachineProgress};
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

struct Display {
    bars: MultiProgress,
    active: Mutex<Vec<Weak<State>>>,
    terminal: bool,
    advised: Mutex<Option<Weak<State>>>,
}

struct State {
    bar: ProgressBar,
    label: String,
    detail: Mutex<String>,
    started: Instant,
    last_report: AtomicU64,
    advice: Mutex<BTreeMap<TaskKey, Task>>,
    parents: Mutex<BTreeMap<TaskKey, TaskKey>>,
    labels: Mutex<BTreeMap<[u8; 32], String>>,
    input: Mutex<Option<[u8; 32]>>,
    machines: Mutex<BTreeMap<String, MachineDisplay>>,
    reported: AtomicBool,
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
    worker_ns: u64,
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
                    if state.bar.is_finished() {
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
    if let Some(display) = DISPLAY.get().filter(|display| display.terminal) {
        let _ = display.bars.println(format!("fix: {}", message.as_ref()));
    } else {
        eprintln!("fix: {}", message.as_ref());
    }
}

pub fn write_stderr(bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let write = || {
        let mut stderr = std::io::stderr().lock();
        stderr.write_all(bytes)?;
        stderr.flush()
    };
    match DISPLAY.get() {
        Some(display) => display.bars.suspend(write),
        None => write(),
    }
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

pub struct Stderr;

impl std::io::Write for Stderr {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        write_stderr(bytes)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
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
        _ => "{prefix} [{elapsed_precise}] {msg}",
    };
    ProgressStyle::with_template(template).unwrap()
}

impl Progress {
    pub fn activity(label: impl Into<String>) -> Self {
        Self::new(format!("local: {}", label.into()), None, false)
    }

    pub fn advised(label: impl Into<String>) -> Self {
        if advised().is_some() {
            return Self::activity(label);
        }
        let progress = Self::new(
            format!("{} [coordinator: local]", label.into()),
            None,
            false,
        );
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
        let bar = display.bars.add(ProgressBar::new_spinner());
        bar.set_style(style(total, bytes));
        if let Some(total) = total {
            bar.set_length(total);
        }
        bar.set_prefix(label.clone());
        let state = Arc::new(State {
            bar,
            label,
            detail: Mutex::new(String::new()),
            started: Instant::now(),
            last_report: AtomicU64::new(0),
            advice: Mutex::new(BTreeMap::new()),
            parents: Mutex::new(BTreeMap::new()),
            labels: Mutex::new(BTreeMap::new()),
            input: Mutex::new(None),
            machines: Mutex::new(BTreeMap::new()),
            reported: AtomicBool::new(false),
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
            if total == 0 || completed > total {
                return;
            }
            if completed == total {
                state.bar.unset_length();
                state.bar.set_style(
                    ProgressStyle::with_template("{prefix} [{elapsed_precise}] {msg}").unwrap(),
                );
                self.message(format!(
                    "{completed}/{total} reported units complete; task still running"
                ));
                return;
            }
            state.bar.set_style(style(Some(total), false));
            state.bar.set_length(total);
            state.bar.set_position(completed);
        }
    }

    pub fn finish(self) {
        if let Some(state) = &self.0 {
            state.bar.finish_and_clear();
            let reported = report_evaluation(state, "completed");
            let child = advised().is_some_and(|root| !Arc::ptr_eq(&root, state));
            if !reported && !child && state.started.elapsed() >= Duration::from_secs(1) {
                println(format!(
                    "{} completed [{:.1}s]",
                    state.label,
                    state.started.elapsed().as_secs_f64()
                ));
            }
        }
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
    println(format!(
        "evaluation {outcome} [{:.2}s]",
        state.started.elapsed().as_secs_f64()
    ));
    for (location, machine) in machines.iter() {
        let stats = &machine.stats;
        println(format!(
            "  {location}: {} applications, {} collector calls; {:.2}s worker time; peak workers {}/{}, queue {}; max sampled VM allocation {}/{} MiB",
            machine.completed,
            machine.privileged_completed,
            machine.worker_ns as f64 / 1e9,
            machine.peak_busy,
            stats.workers,
            machine.peak_queue,
            machine.peak_memory / (1 << 20),
            stats.memory_total / (1 << 20)
        ));
    }
    true
}

pub fn evaluation_progress(snapshot: EvaluationSnapshot) {
    let Some(state) = advised() else { return };
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
            .or_insert_with(|| MachineDisplay {
                progress: Progress::new(format!("  {location} Fix runtime"), None, false),
                tasks: BTreeMap::new(),
                stats: stats.clone(),
                peak_busy: 0,
                peak_queue: 0,
                peak_memory: 0,
                completed: 0,
                privileged_completed: 0,
                worker_ns: 0,
            });
        machine.completed += stats
            .completed
            .checked_sub(machine.stats.completed)
            .unwrap_or(stats.completed);
        machine.privileged_completed += stats
            .privileged_completed
            .checked_sub(machine.stats.privileged_completed)
            .unwrap_or(stats.privileged_completed);
        machine.worker_ns += stats
            .worker_ns
            .checked_sub(machine.stats.worker_ns)
            .unwrap_or(stats.worker_ns);
        machine.peak_busy = machine.peak_busy.max(stats.peak_busy_workers);
        machine.peak_queue = machine.peak_queue.max(stats.peak_queued);
        machine.peak_memory = machine.peak_memory.max(stats.memory_used);
        machine.progress.message(format!(
            "workers {}/{}, queue {}, VM allocation {}/{} MiB, {} applications, {} collector calls complete",
            stats.busy_workers,
            stats.workers,
            stats.queued,
            stats.memory_used / (1 << 20),
            stats.memory_total / (1 << 20),
            stats.completed,
            stats.privileged_completed
        ));
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
                    progress: Progress::new(
                        format!(
                            "{}{}: {}",
                            "  ".repeat(depth),
                            location,
                            reported_title(entry, &stats, &labels)
                        ),
                        None,
                        false,
                    ),
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
                if let Some((completed, total)) = entry.progress {
                    task.progress.position(completed, total);
                }
            }
            machine.tasks.retain(|id, _| active.contains(id));
        }
        machine.stats = stats;
        let busy: u64 = machines
            .values()
            .map(|machine| machine.stats.busy_workers)
            .sum();
        let queued: u64 = machines.values().map(|machine| machine.stats.queued).sum();
        let completed: u64 = machines.values().map(|machine| machine.completed).sum();
        *state.detail.lock().unwrap() = format!(
            "{busy} workers busy, {queued} queued, {completed} applications complete across {} Fix runtimes",
            machines.len()
        );
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
    let parent = state.parents.lock().unwrap().get(&id).copied();
    let parent_bar = parent
        .and_then(|id| tasks.get(&id))
        .and_then(|task| task.progress.0.as_ref())
        .map(|state| state.bar.clone())
        .unwrap_or_else(|| state.bar.clone());
    tasks.entry(id).or_insert_with(|| {
        let title = String::new();
        let display_title = task_title(id.0, &title, id.1, &state.labels.lock().unwrap());
        let progress = Progress::new(
            format!("{}local: {display_title}", "  ".repeat(depth)),
            None,
            false,
        );
        if let (Some(display), Some(state)) = (DISPLAY.get(), &progress.0) {
            display.bars.remove(&state.bar);
            display.bars.insert_after(&parent_bar, state.bar.clone());
        }
        Task {
            progress,
            title,
            operation: id.1,
            depth,
        }
    })
}

pub fn advice(id: [u8; 32], operation: Option<EvaluationOperation>, key: &[u8], payload: &[u8]) {
    let Some(state) = advised() else { return };
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
            let task = task(&state, &mut tasks, (id, operation));
            task.progress.position(completed, total);
        }
        _ => (),
    }
}

pub fn tree_progress(id: [u8; 32], children: &[[u8; 32]], completed: u64, total: u64) {
    let Some(state) = advised() else { return };
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
    if !named && !root {
        return;
    }
    let mut tasks = state.advice.lock().unwrap();
    if completed == total {
        tasks.remove(&(id, Some(EvaluationOperation::Evaluate)));
    } else {
        let task = task(
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
        if let Some(state) = &self.0 {
            report_evaluation(state, "stopped");
            if !state.bar.is_finished() {
                state.bar.abandon();
            }
            if let Some(display) = DISPLAY.get() {
                display.bars.remove(&state.bar);
            }
        }
    }
}
