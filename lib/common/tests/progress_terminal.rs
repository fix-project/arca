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
    for mode in ["complete", "stop"] {
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
        let mut child = Command::new(&probe)
            .arg(mode)
            .env("TERM", "xterm-256color")
            .env("NO_COLOR", "1")
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave))
            .spawn()
            .unwrap();
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
        let visible = screen(&output);
        assert!(visible.contains("result preserved"), "{mode}: {visible:?}");
        assert!(
            visible.contains("diagnostic preserved"),
            "{mode}: {visible:?}"
        );
        assert!(
            visible.contains("✓ local: Apply named computation"),
            "{mode}: {visible:?}"
        );
        assert!(
            visible.contains("final diagnostic\nfix: finished"),
            "{mode}: {visible:?}"
        );
        assert!(!visible.contains("ephemeral"), "{mode}: {visible:?}");
        assert!(!visible.contains("work in progress"), "{mode}: {visible:?}");
    }
}
