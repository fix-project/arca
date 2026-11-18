use std::{
    fs::File,
    io::Read,
    os::fd::{FromRawFd, OwnedFd},
    path::PathBuf,
    process::{Command, Stdio},
};

fn screen(output: &str) -> String {
    let mut rows: Vec<Vec<char>> = vec![Vec::new()];
    let (mut x, mut y) = (0usize, 0usize);
    let mut chars = output.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => {
                assert_eq!(chars.next(), Some('['));
                let mut parameters = String::new();
                let command = loop {
                    let ch = chars.next().unwrap();
                    if ch.is_ascii_alphabetic() {
                        break ch;
                    }
                    parameters.push(ch);
                };
                let values: Vec<usize> = parameters
                    .split(';')
                    .map(|value| value.parse().unwrap_or(0))
                    .collect();
                let amount = values[0].max(1);
                match command {
                    'A' => y = y.saturating_sub(amount),
                    'B' => y += amount,
                    'C' => x += amount,
                    'D' => x = x.saturating_sub(amount),
                    'G' => x = amount - 1,
                    'H' | 'f' => {
                        y = amount - 1;
                        x = values.get(1).copied().unwrap_or(1).max(1) - 1;
                    }
                    'K' => {
                        rows.resize_with(y + 1, Vec::new);
                        if values[0] == 2 {
                            rows[y].clear();
                        } else {
                            rows[y].truncate(x);
                        }
                    }
                    'J' => {
                        rows.resize_with(y + 1, Vec::new);
                        rows[y].truncate(x);
                        rows.truncate(y + 1);
                    }
                    'h' | 'l' | 'm' => (),
                    _ => panic!("unsupported terminal command {parameters}{command}"),
                }
            }
            '\r' => x = 0,
            '\n' => y += 1,
            ch => {
                if x == 100 {
                    x = 0;
                    y += 1;
                }
                rows.resize_with(y + 1, Vec::new);
                let width = rows[y].len().max(x + 1);
                rows[y].resize(width, ' ');
                rows[y][x] = ch;
                x += 1;
            }
        }
    }
    rows.into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn diagnostics_and_completed_steps_survive_terminal_cleanup() {
    let probe = PathBuf::from(std::env::var_os("TEST_SRCDIR").unwrap())
        .join(std::env::var_os("TEST_WORKSPACE").unwrap())
        .join(env!("PROBE"));
    for (mode, summarize, color, no_abbrev) in [
        ("complete", true, false, false),
        ("stop", true, false, false),
        ("complete", false, false, false),
        ("complete", false, true, false),
        ("complete", false, false, true),
    ] {
        let (mut master, mut slave) = (-1, -1);
        let size = libc::winsize {
            ws_row: 40,
            ws_col: 100,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    core::ptr::null_mut(),
                    core::ptr::null(),
                    &size,
                )
            },
            0
        );
        let mut terminal = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        let mut command = Command::new(&probe);
        command.arg(mode);
        if summarize {
            command.arg("--summarize");
        }
        if no_abbrev {
            command.arg("--no-abbrev");
        }
        command.env("TERM", "xterm-256color");
        if color {
            command.env_remove("NO_COLOR");
        } else {
            command.env("NO_COLOR", "1");
        }
        let mut child = command
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave))
            .spawn()
            .unwrap();
        drop(command);
        let mut output = Vec::new();
        let mut bytes = [0; 4096];
        loop {
            match terminal.read(&mut bytes) {
                Ok(0) => break,
                Ok(length) => output.extend_from_slice(&bytes[..length]),
                Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
                Err(error) => panic!("reading terminal: {error}"),
            }
        }
        assert!(child.wait().unwrap().success());
        let output = String::from_utf8(output).unwrap();
        assert!(
            output.contains("ephemeral"),
            "{mode}: no live progress was drawn"
        );
        let activity = "ephemeral unmeasured work";
        let indicators = output
            .match_indices(activity)
            .filter_map(|(index, _)| {
                let frame = screen(&output[..index + activity.len()]);
                frame
                    .lines()
                    .find(|line| line.contains(activity))
                    .and_then(|line| line.trim_start().chars().next())
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            indicators.len() > 1,
            "{mode}: unmeasured work needs an animated indicator: {output:?}"
        );
        for completed in [1, 2] {
            let counter = format!("] {completed}/3");
            let end = output
                .find(&counter)
                .unwrap_or_else(|| panic!("{output:?}"));
            let start = output[..end].rfind('[').unwrap();
            assert!(end > start + 1, "{mode}: measured work needs a visible bar");
        }
        assert!(
            !output.contains("0/3"),
            "{mode}: work awaiting its first completion needs a spinner: {output:?}"
        );
        for (index, activity) in output.match_indices("Evaluate named tree") {
            let frame = screen(&output[..index + activity.len()]);
            assert!(
                !frame.contains("[2;3m") && !frame.contains("[0m"),
                "{mode}: styling leaked into live text: {frame:?}"
            );
        }
        let next = output.split_once("ephemeral next execution").unwrap().1;
        assert!(
            !next.contains("1/3") && !next.contains("2/3"),
            "{mode}: counters must belong to the current execution: {next:?}"
        );
        let visible = screen(&output);
        for node in ["local (3 running)", "origin (2 running)"] {
            assert!(
                output.contains(node),
                "{mode}: concurrent work on each node must be visible: {output:?}"
            );
            assert!(
                !visible.contains(node),
                "{mode}: live node rows must be cleared: {visible:?}"
            );
        }
        assert!(
            !visible.contains("[2;3m") && !visible.contains("[0m"),
            "{mode}: styling leaked into visible text: {visible:?}"
        );
        assert!(visible.contains("result preserved"), "{mode}: {visible:?}");
        assert!(
            visible.contains("diagnostic preserved"),
            "{mode}: {visible:?}"
        );
        assert!(
            visible.contains("[1] Connect origin (4 workers)"),
            "{mode}: {visible:?}"
        );
        assert!(
            visible.contains("[3] local: Apply named computation"),
            "{mode}: {visible:?}"
        );
        assert!(
            visible.contains("final diagnostic\nfix: finished"),
            "{mode}: {visible:?}"
        );
        let input = if no_abbrev {
            "02".repeat(32)
        } else {
            "named input".into()
        };
        assert!(
            visible.contains(&format!("[2] local: Load {input} (from origin)")),
            "{mode}: {visible:?}"
        );
        assert!(
            visible.contains("[4] FAILED origin: Load generated result (from local)"),
            "{mode}: {visible:?}"
        );
        assert!(
            visible.contains("[5] local: Load resident input (0.00s)"),
            "{mode}: {visible:?}"
        );
        let coupon = if no_abbrev {
            "07".repeat(32)
        } else {
            "Eval(named input, named result)".into()
        };
        assert!(
            visible.contains(&format!("[6] local: Load {coupon} (from origin)")),
            "{mode}: {visible:?}"
        );
        if summarize {
            let lines: Vec<_> = visible.lines().collect();
            let header = lines
                .iter()
                .position(|line| line.starts_with("Node "))
                .unwrap();
            assert!(
                header > 0 && lines[header - 1].trim().is_empty(),
                "{mode}: summary is not separated: {visible:?}"
            );
            assert!(
                visible.contains("Mem peak") && visible.contains("Received"),
                "{visible:?}"
            );
        } else {
            assert!(
                !visible.contains("Mem peak"),
                "summary requires --summarize: {visible:?}"
            );
        }
        assert!(!visible.contains("ephemeral"), "{mode}: {visible:?}");
        assert!(!visible.contains("work in progress"), "{mode}: {visible:?}");
    }
}
