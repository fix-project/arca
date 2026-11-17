use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    collections::BTreeMap,
    io::IsTerminal,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

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
    advice: Mutex<BTreeMap<[u8; 32], Task>>,
    parents: Mutex<BTreeMap<[u8; 32], [u8; 32]>>,
}

struct Task {
    progress: Progress,
    title: String,
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
        Self::new(label.into(), None, false)
    }

    pub fn advised(label: impl Into<String>) -> Self {
        let progress = Self::activity(label);
        if let (Some(display), Some(state)) = (DISPLAY.get(), &progress.0) {
            *display.advised.lock().unwrap() = Some(Arc::downgrade(state));
        }
        progress
    }

    pub fn items(label: impl Into<String>, total: usize) -> Self {
        Self::new(label.into(), Some(total as u64), false)
    }

    pub fn bytes(label: impl Into<String>, total: u64) -> Self {
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
        });
        display.active.lock().unwrap().push(Arc::downgrade(&state));

        Self(Some(state))
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
            if state.started.elapsed() >= Duration::from_secs(1) {
                println(format!(
                    "{} completed [{:.1}s]",
                    state.label,
                    state.started.elapsed().as_secs_f64()
                ));
            }
        }
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

fn task<'a>(state: &State, tasks: &'a mut BTreeMap<[u8; 32], Task>, id: [u8; 32]) -> &'a mut Task {
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
        let title = task_name(&id);
        let progress = Progress::activity(format!("{}{title}", "  ".repeat(depth)));
        if let (Some(display), Some(state)) = (DISPLAY.get(), &progress.0) {
            display.bars.remove(&state.bar);
            display.bars.insert_after(&parent_bar, state.bar.clone());
        }
        Task {
            progress,
            title,
            depth,
        }
    })
}

pub fn advice(id: [u8; 32], key: &[u8], payload: &[u8]) {
    let Some(state) = advised() else { return };
    let mut tasks = state.advice.lock().unwrap();
    match key {
        b"fix::title" => {
            let Ok(title) = std::str::from_utf8(payload) else {
                return;
            };
            let task = task(&state, &mut tasks, id);
            task.title = title
                .chars()
                .filter(|character| !character.is_control())
                .collect();
            *state.detail.lock().unwrap() = format!("running {}", task.title);
            if let Some(state) = &task.progress.0 {
                state
                    .bar
                    .set_prefix(format!("{}{}", "  ".repeat(task.depth), task.title));
            }
        }
        b"fix::progress" if payload.len() == 16 => {
            let completed = u64::from_le_bytes(payload[..8].try_into().unwrap());
            let total = u64::from_le_bytes(payload[8..].try_into().unwrap());
            if completed > total {
                return;
            }
            let task = task(&state, &mut tasks, id);
            task.progress.position(completed, total);
        }
        _ => (),
    }
}

pub fn tree_progress(id: [u8; 32], children: &[[u8; 32]], completed: u64, total: u64) {
    let Some(state) = advised() else { return };
    for child in children {
        state.parents.lock().unwrap().entry(*child).or_insert(id);
    }
    let mut tasks = state.advice.lock().unwrap();
    if completed == total {
        tasks.remove(&id);
    } else {
        let task = task(&state, &mut tasks, id);
        task.progress.message("evaluating Tree children");
        task.progress.position(completed, total);
    }
}

pub fn task_finished(id: [u8; 32]) {
    if let Some(state) = advised() {
        let mut tasks = state.advice.lock().unwrap();
        tasks.remove(&id);
        *state.detail.lock().unwrap() = tasks
            .values()
            .next()
            .map(|task| format!("running {}", task.title))
            .unwrap_or_default();
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        if let Some(state) = &self.0 {
            if !state.bar.is_finished() {
                state.bar.abandon();
            }
            if let Some(display) = DISPLAY.get() {
                display.bars.remove(&state.bar);
            }
        }
    }
}
