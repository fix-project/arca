use fixremote_testing::Agent;
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("TEST_SRCDIR").unwrap())
        .join(std::env::var_os("TEST_WORKSPACE").unwrap())
        .join(name)
}

struct Machine {
    directory: PathBuf,
    agent: Agent,
}
impl Machine {
    fn new(root: &Path, name: &str, seed: u8) -> Self {
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).unwrap();
        Self {
            agent: Agent::new(&directory.join("agent"), seed),
            directory,
        }
    }
    fn run(&self, cwd: &Path, arguments: &[&str], ssh: Option<&Path>) -> Output {
        self.run_with_jobs(cwd, arguments, ssh, 4)
    }
    fn run_with_jobs(
        &self,
        cwd: &Path,
        arguments: &[&str],
        ssh: Option<&Path>,
        jobs: usize,
    ) -> Output {
        let mut command = Command::new(fixture(env!("FIX")));
        command
            .args(["-j", &jobs.to_string(), "--memory", "2048"])
            .args(arguments)
            .current_dir(cwd)
            .env("FIX_CONFIG_DIR", self.directory.join("config"))
            .env("XDG_DATA_HOME", self.directory.join("data"))
            .env("SSH_AUTH_SOCK", &self.agent.socket)
            .env_remove("FIX_UNSIGNED")
            .env_remove("FIX_TRUST")
            .env_remove("FIX_SCHEDULER")
            .env_remove("FIX_COLLECTOR")
            .env_remove("FIX_PARSER")
            .env_remove("FIX_COMPILER")
            .env_remove("BUILD_WORKSPACE_DIRECTORY");
        if let Some(ssh) = ssh {
            command.env(
                "PATH",
                format!("{}:{}", ssh.display(), std::env::var("PATH").unwrap()),
            );
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
    fn bootstrap(&self) {
        self.run(&self.directory, &["trust", "add-agent"], None);
        self.run(
            &self.directory,
            &["bootstrap", fixture(env!("SEED")).to_str().unwrap()],
            None,
        );
    }
}

#[test]
fn cloned_compilation_thunks_can_be_applied() {
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = std::env::temp_dir().join(format!("fix-remote-compile-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let _cleanup = Directory(root.clone());
    let source = Machine::new(&root, "source", 61);
    let destination = Machine::new(&root, "destination", 62);
    source.bootstrap();
    destination.bootstrap();
    source.run(
        &source.directory,
        &["trust", "add-key", &destination.agent.public_key],
        None,
    );
    destination.run(
        &destination.directory,
        &["trust", "add-key", &source.agent.public_key],
        None,
    );
    let repository = source.directory.join("repo");
    std::fs::create_dir(&repository).unwrap();
    source.run(&repository, &["init"], None);
    source.run(
        &repository,
        &["pull", "--exact", "bootstrap", "compile"],
        None,
    );
    let module = fixture(env!("MODULE"));
    let wasm = source.run(
        &repository,
        &["create-blob", module.to_str().unwrap()],
        None,
    );
    let wasm_name = String::from_utf8(wasm.stdout).unwrap();
    let wasm_name = wasm_name.trim();
    source.run(
        &repository,
        &["label", "set", "addblob.wasm", wasm_name],
        None,
    );
    source.run(
        &repository,
        &[
            "-l",
            "addblob",
            "#(\"\" *'@label(\"bootstrap:compile\") @label(\"addblob.wasm\"))",
        ],
        None,
    );

    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let quote = |path: &Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
    let script = format!(
        "#!/bin/sh\nexport FIX_CONFIG_DIR={} XDG_DATA_HOME={} SSH_AUTH_SOCK={}\nexec {} -C {} -j 4 --memory 2048 serve --stdio\n",
        quote(&source.directory.join("config")),
        quote(&source.directory.join("data")),
        quote(&source.agent.socket),
        quote(&fixture(env!("FIX"))),
        quote(&repository)
    );
    let ssh = bin.join("ssh");
    std::fs::write(&ssh, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();

    destination.run(
        &destination.directory,
        &["clone", "source:/repo", "repo"],
        Some(&bin),
    );
    let cloned = destination.directory.join("repo");
    let evaluated = destination.run_with_jobs(
        &cloned,
        &["*#(\"\" *#(\"\" *'@label(\"origin:bootstrap:compile\") *'@label(\"origin:addblob.wasm\")) 20u64 22u64)"],
        Some(&bin),
        1,
    );
    let diagnostics = String::from_utf8_lossy(&evaluated.stderr);
    eprintln!("{diagnostics}");
    assert!(
        diagnostics.contains("origin: Apply compile Wasm"),
        "{diagnostics}"
    );
    let data = format!("{}ffff", &wasm_name[..60]);
    assert!(
        !cloned.join(".fix/data").join(data).exists(),
        "{diagnostics}"
    );
    let result = String::from_utf8(evaluated.stdout).unwrap();
    let value = destination.run(&cloned, &["read-blob", "--u64", result.trim()], None);
    assert_eq!(String::from_utf8(value.stdout).unwrap().trim(), "42");
    let evaluated = destination.run_with_jobs(
        &cloned,
        &[
            "--summarize",
            "*#(\"\" *@label(\"origin:addblob\") 1u64 2u64)",
        ],
        Some(&bin),
        0,
    );
    let diagnostics = String::from_utf8_lossy(&evaluated.stderr);
    let row = diagnostics
        .lines()
        .find(|line| line.starts_with("local "))
        .unwrap();
    let fields: Vec<_> = row.split_whitespace().collect();
    assert_eq!(&fields[1..3], &["0", "0"], "{diagnostics}");
    let result = String::from_utf8(evaluated.stdout).unwrap();
    let value = destination.run(&cloned, &["read-blob", "--u64", result.trim()], None);
    assert_eq!(String::from_utf8(value.stdout).unwrap().trim(), "3");
}
