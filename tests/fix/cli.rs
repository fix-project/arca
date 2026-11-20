use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn runfiles() -> PathBuf {
    PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap())
}

fn seed_tool(directory: &Path, arguments: &[String]) {
    let output = Command::new(runfiles().join(env!("FIX_SEED")))
        .args(arguments)
        .env("FIX_UNSIGNED", "true")
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn program(name: &str, path: &str) -> String {
    format!("{name}={}", runfiles().join(path).display())
}

fn seed_with(directory: &Path, wasm2c: &str, clang: &str, programs: &[(&str, &str)]) {
    let toolchain = directory.join(".toolchain");
    std::fs::create_dir_all(&toolchain).unwrap();
    let mut arguments: Vec<String> = ["create", "."].map(String::from).into();
    for path in [env!("FIX_COMPILE"), wasm2c, clang] {
        arguments.push(runfiles().join(path).display().to_string());
    }
    arguments.push(toolchain.display().to_string());
    arguments.push(program("evaluator", env!("FIX_SCHEDULER")));
    arguments.push(program("coupon-collector", env!("FIX_COLLECTOR")));
    arguments.extend(programs.iter().map(|(name, path)| program(name, path)));
    seed_tool(directory, &arguments);
    std::fs::remove_dir(&toolchain).unwrap();
}

fn seed(directory: &Path, programs: &[(&str, &str)]) {
    seed_with(
        directory,
        env!("FIX_IDENTITY"),
        env!("FIX_IDENTITY"),
        programs,
    );
}

fn trust_file() -> &'static Path {
    static TRUST: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    TRUST.get_or_init(|| {
        let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("authority");
        std::fs::create_dir_all(&directory).unwrap();
        seed(&directory, &[]);
        let output = Command::new(runfiles().join(env!("FIX_CLI")))
            .args(["trust", "add-tagger", "coupon-collector"])
            .env("FIX_UNSIGNED", "true")
            .env("FIX_CONFIG_DIR", configuration())
            .current_dir(&directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        directory.join(".fix/trust")
    })
}

fn configuration() -> PathBuf {
    PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("user-config")
}

fn command(directory: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(runfiles().join(env!("FIX_CLI")));
    command
        .args(arguments)
        .env("FIX_UNSIGNED", "true")
        .current_dir(directory)
        .env("FIX_JOBS", "1")
        .env("FIX_TRUST", trust_file())
        .env("FIX_CONFIG_DIR", configuration())
        .env(
            "XDG_DATA_HOME",
            PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("data"),
        )
        .env("FIX_SCHEDULER", "evaluator")
        .env("FIX_COLLECTOR", "coupon-collector")
        .env_remove("BUILD_WORKSPACE_DIRECTORY");
    command
}

fn run(directory: &Path, arguments: &[&str], input: &[u8], status: i32) -> Vec<u8> {
    let mut child = command(directory, arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(status),
        "{arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    if status == 2 {
        assert!(output.stdout.is_empty());
    }
    output.stdout
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap().trim_end().into()
}

fn value(directory: &Path, arguments: &[&str]) -> String {
    text(run(directory, arguments, b"", 0))
}

fn claim(directory: &Path, coupon: &str) -> Vec<String> {
    let fields = value(directory, &["read-tree", coupon]);
    let reference = fields.lines().nth(1).unwrap();
    let flags = u8::from_str_radix(&reference[62..], 16).unwrap();
    assert_eq!(flags & 0x1f, 0x10, "{coupon}: {fields}");
    let object = format!("{}{:02x}", &reference[..62], flags & 0xe0);
    value(directory, &["read-tree", &object])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Requests one scheduler mode through `schedule`, returning its raw result.
fn evaluate_raw(
    directory: &Path,
    mode: &str,
    scheduler: &str,
    input: &str,
    globals: &[&str],
    status: i32,
) -> String {
    let mode = value(directory, &["create-blob", "--string", mode]);
    let request = value(
        directory,
        &[
            "create-tree",
            &value(directory, &["create-blob", "--u64", "2147483648"]),
            scheduler,
            &mode,
            "coupon-collector",
            input,
        ],
    );
    let mut arguments = globals.to_vec();
    arguments.extend(["schedule", "--trusted-tagger", "coupon-collector", &request]);
    text(run(directory, &arguments, b"", status))
}

fn evaluate(
    directory: &Path,
    mode: &str,
    scheduler: &str,
    input: &str,
    globals: &[&str],
    status: i32,
) -> String {
    let output = evaluate_raw(directory, mode, scheduler, input, globals, status);
    if status != 0 || globals.contains(&"--memoize=never") {
        return output;
    }
    let claim = claim(directory, &output);
    assert_eq!(claim[1], input);
    claim[2].clone()
}

fn eval(directory: &Path, input: &str) -> String {
    evaluate(directory, "eval", "evaluator", input, &[], 0)
}

fn think(directory: &Path, input: &str) -> String {
    evaluate(directory, "think", "evaluator", input, &[], 0)
}

fn compiler(directory: &Path) -> String {
    let closure = value(directory, &["read-tree", "compile"]);
    let fixpoint = value(directory, &["read-tree", closure.lines().nth(1).unwrap()]);
    fixpoint.lines().nth(1).unwrap().to_owned()
}

fn archive(directory: &Path, values: &[&str], path: &Path) {
    let mut arguments = vec!["visit", "--paths"];
    arguments.extend_from_slice(values);
    let paths = run(directory, &arguments, b"", 0);
    let repository = PathBuf::from(value(directory, &["repository"]));
    let headers = directory.join(".archive-headers");
    for name in ["data", "objects", "labels"] {
        std::fs::create_dir_all(headers.join(".fix").join(name)).unwrap();
    }
    let mut child = Command::new("tar")
        .arg("--create")
        .arg("--file")
        .arg(path)
        .args(["--no-recursion", "--directory"])
        .arg(&headers)
        .args([".fix/data", ".fix/objects", ".fix/labels", "--directory"])
        .arg(repository.parent().unwrap())
        .args(["--null", "--files-from", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&paths).unwrap();
    assert!(child.wait().unwrap().success());
    std::fs::remove_dir_all(headers).unwrap();
}

fn compilation(directory: &Path, source: &str) -> String {
    let metadata = value(directory, &["create-blob", "--u64", "536870912"]);
    let units = value(directory, &["create-blob", "--u64", "1"]);
    let combination = value(
        directory,
        &["create-tree", &metadata, "compile", source, &units],
    );
    let thunk = value(directory, &["create-application-thunk", &combination]);
    value(directory, &["create-strict-encode", &thunk])
}

fn compiler_fixture(directory: &Path) {
    std::fs::create_dir(directory).unwrap();
    seed_with(
        directory,
        env!("FIX_COMPILER_TOOL"),
        env!("FIX_COMPILER_TOOL"),
        &[("identity", env!("FIX_IDENTITY"))],
    );
}

#[test]
fn compilation_evaluates_only_required_tool_outputs() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("compiler_outputs");
    compiler_fixture(&directory);
    let source = value(&directory, &["create-blob", "--string", "source"]);
    let runnable = eval(&directory, &compilation(&directory, &source));
    run(&directory, &["is-tag", &runnable], b"", 0);
    let fields = value(&directory, &["read-tree", &runnable]);
    let fields: Vec<_> = fields.lines().collect();
    assert_eq!(
        fields[0],
        value(&directory, &["resolve", "trusted-compiler"])
    );
    assert_eq!(
        run(&directory, &["read-blob", fields[1]], b"", 0),
        b"program"
    );
    assert_eq!(
        run(&directory, &["read-blob", fields[2]], b"", 0),
        b"Runnable"
    );
}

#[test]
fn runnable_trust_configuration() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("trust");
    std::fs::create_dir(&directory).unwrap();
    seed(&directory, &[("identity", env!("FIX_IDENTITY"))]);
    let metadata = value(&directory, &["create-blob", "--u64", "536870912"]);
    let reply = value(&directory, &["create-blob", "--string", "reply"]);
    let combination = value(&directory, &["create-tree", &metadata, "identity", &reply]);
    let application = value(&directory, &["create-application-thunk", &combination]);
    let config = directory.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::copy(
        configuration().join("trusted_taggers"),
        config.join("trusted_taggers"),
    )
    .unwrap();
    let request = |mode: &str, input: &str| {
        let mode = value(&directory, &["create-blob", "--string", mode]);
        value(
            &directory,
            &[
                "create-tree",
                &value(&directory, &["create-blob", "--u64", "536870912"]),
                "evaluator",
                &mode,
                "coupon-collector",
                input,
            ],
        )
    };
    let schedule = |directory: &Path, memoize: &str, request: &str, trust: Option<&Path>| {
        let mut command = command(
            directory,
            &[
                memoize,
                "schedule",
                "--trusted-tagger",
                "coupon-collector",
                request,
            ],
        );
        command
            .env_remove("FIX_TRUST")
            .env("FIX_CONFIG_DIR", &config);
        if let Some(trust) = trust {
            command.env("FIX_TRUST", trust);
        }
        command.output().unwrap()
    };
    let think = request("think", &application);
    let denied = schedule(&directory, "--memoize=never", &think, None);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("untrusted author"));
    std::fs::copy(trust_file(), config.join("trusted_compilers")).unwrap();
    let accepted = schedule(&directory, "--memoize=never", &think, None);
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert_eq!(text(accepted.stdout), combination);
    let override_path = directory.join("override");
    std::fs::write(&override_path, "").unwrap();
    assert!(
        !schedule(&directory, "--memoize=never", &think, Some(&override_path))
            .status
            .success()
    );
    std::fs::write(&override_path, "invalid authority").unwrap();
    assert!(
        !schedule(&directory, "--memoize=never", &think, Some(&override_path))
            .status
            .success()
    );
    std::fs::remove_file(&override_path).unwrap();
    let missing = schedule(&directory, "--memoize=never", &think, Some(&override_path));
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("cannot read trust file"));
    let bare = value(
        &directory,
        &["create-tree", &metadata, &compiler(&directory), &reply],
    );
    let bare = value(&directory, &["create-application-thunk", &bare]);
    assert!(
        !schedule(
            &directory,
            "--memoize=never",
            &request("think", &bare),
            None
        )
        .status
        .success()
    );
}

#[test]
fn values_persist_across_commands_and_archive_import() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("plumbing");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    let bytes = [7; 64];
    let blob = text(run(&directory, &["create-blob", "-"], &bytes, 0));
    let root = value(&directory, &["-l", "saved", "create-tree", &blob, &blob]);
    assert_eq!(value(&directory, &["resolve", "saved"]), root);
    assert_eq!(
        value(&directory, &["read-tree", &root]),
        format!("{blob}\n{blob}")
    );
    let path = directory.join("values.tar");
    archive(&directory, &[&root], &path);
    let imported = directory.join("imported");
    std::fs::create_dir(&imported).unwrap();
    run(&imported, &["init"], b"", 0);
    run(&imported, &["import", path.to_str().unwrap()], b"", 0);
    assert_eq!(
        value(&imported, &["read-tree", &root]),
        format!("{blob}\n{blob}")
    );
    assert_eq!(run(&imported, &["read-blob", &blob], b"", 0), bytes);
}

#[test]
fn persistent_continuation() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("continuation");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(&directory, &[("continuation", env!("FIX_CONTINUATION"))]);
    let program = value(&directory, &["resolve", "continuation"]);
    let resources = value(&directory, &["create-blob", "--u64", "2147483648"]);
    let mode = value(&directory, &["create-blob", "--u64", "0"]);
    let combination = value(&directory, &["create-tree", &resources, &program, &mode]);
    let application = value(&directory, &["create-application-thunk", &combination]);
    let request = think(&directory, &application);
    assert_eq!(request, think(&directory, &application));
    let fields = value(&directory, &["read-tree", &request]);
    let continuation = fields.lines().nth(1).unwrap();
    for reply in [41u64, 7] {
        let argument = value(&directory, &["create-blob", "--u64", &reply.to_string()]);
        let combination = value(
            &directory,
            &["create-tree", &resources, continuation, &argument],
        );
        let application = value(&directory, &["create-application-thunk", &combination]);
        let result = think(&directory, &application);
        assert_eq!(
            value(&directory, &["read-blob", "--u64", &result]),
            (reply + 1).to_string()
        );
    }
    let mode = value(&directory, &["create-blob", "--u64", "3"]);
    let original = value(&directory, &["create-tree", &resources, &program, &mode]);
    let application = value(&directory, &["create-application-thunk", &original]);
    let request = think(&directory, &application);
    let fields = value(&directory, &["read-tree", &request]);
    let continuation = fields.lines().nth(1).unwrap();
    let argument = value(&directory, &["create-blob", "--u64", "0"]);
    let resumed = value(
        &directory,
        &["create-tree", &resources, continuation, &argument],
    );
    let application = value(&directory, &["create-application-thunk", &resumed]);
    let tag = think(&directory, &application);
    let fields = value(&directory, &["read-tree", &tag]);
    assert_eq!(fields.lines().next().unwrap(), original);
}
