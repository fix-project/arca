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
        directory.join(".fix/trust")
    })
}

fn command(directory: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(runfiles().join(env!("FIX_CLI")));
    command
        .args(arguments)
        .current_dir(directory)
        .env("FIX_JOBS", "1")
        .env("FIX_TRUST", trust_file())
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

fn assert_error(directory: &Path, result: &str, marker: &str, payload: &[u8]) {
    run(directory, &["is-tag", result], b"", 1);
    let fields = value(directory, &["read-tree", result]);
    let fields: Vec<_> = fields.lines().collect();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        run(directory, &["read-blob", fields[0]], b"", 0),
        marker.as_bytes()
    );
    assert_eq!(run(directory, &["read-blob", fields[1]], b"", 0), payload);
}

fn assert_trap(directory: &Path, result: &str, code: u32) {
    assert_error(directory, result, "Wasm.trap", &code.to_le_bytes());
}

fn assert_panic(directory: &Path, result: &str, message: &str) {
    assert_error(directory, result, "Rust.panic", message.as_bytes());
}

fn claim(directory: &Path, coupon: &str) -> Vec<String> {
    let fields = value(directory, &["read-tree", coupon]);
    let reference = fields.lines().nth(1).unwrap();
    let flags = u8::from_str_radix(&reference[62..], 16).unwrap();
    assert_eq!(flags & 0x1f, 0x10);
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
        &["create-tree", &mode, scheduler, "coupon-collector", input],
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

fn stored_claims(directory: &Path, prefix: &[&str]) -> Vec<String> {
    let fields: Vec<_> = prefix
        .iter()
        .map(|name| {
            let bytes = std::array::from_fn(|index| {
                u8::from_str_radix(&name[index * 2..index * 2 + 2], 16).unwrap()
            });
            unsafe { fixhandle::Handle::parse(bytes) }.unwrap()
        })
        .collect();
    fixrepository::Repository::open(directory.join(".fix").to_str().unwrap())
        .unwrap()
        .claims(&fields)
        .unwrap()
        .iter()
        .map(|tree| tree.as_handle().to_string())
        .collect()
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
    let metadata = value(directory, &["create-blob", "--string", ""]);
    let units = value(directory, &["create-blob", "--u64", "1"]);
    let combination = value(
        directory,
        &["create-tree", &metadata, "compile", source, &units],
    );
    let thunk = value(directory, &["create-application-thunk", &combination]);
    value(directory, &["create-strict-encode", &thunk])
}

#[test]
fn seed_labels_support_tools_and_directory_contents() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("seed_labels");
    std::fs::create_dir(&directory).unwrap();
    seed(
        &directory,
        &[
            ("lld", env!("FIX_IDENTITY")),
            ("wasm-merge", env!("FIX_IDENTITY")),
        ],
    );
    let sysroot = directory.join("sysroot");
    std::fs::create_dir_all(sysroot.join("include")).unwrap();
    std::fs::write(sysroot.join("include/marker.h"), b"int marker;").unwrap();
    let adapter = directory.join("adapter.wasm");
    std::fs::write(&adapter, b"\0asm\x01\0\0\0").unwrap();
    seed_tool(
        &directory,
        &[
            "add".into(),
            ".".into(),
            "--blob".into(),
            format!("flatware={}", adapter.display()),
            "--directory".into(),
            format!("wasip1-sysroot={}", sysroot.display()),
        ],
    );
    let author = value(&directory, &["resolve", "trusted-compiler"]);
    for name in ["clang", "lld", "wasm2c", "wasm-merge"] {
        let fields = value(&directory, &["read-tree", name]);
        let fields = fields.lines().collect::<Vec<_>>();
        assert_eq!(fields[0], author);
        assert_eq!(
            run(&directory, &["read-blob", fields[2]], b"", 0),
            b"Runnable"
        );
        assert_eq!(
            run(&directory, &["read-blob", fields[1]], b"", 0),
            std::fs::read(runfiles().join(env!("FIX_IDENTITY"))).unwrap()
        );
    }
    assert_eq!(
        run(&directory, &["read-blob", "flatware"], b"", 0),
        b"\0asm\x01\0\0\0"
    );
    let entries = value(&directory, &["read-tree", "wasip1-sysroot"]);
    let include = value(&directory, &["read-tree", entries.lines().next().unwrap()]);
    let fields = include.lines().collect::<Vec<_>>();
    assert_eq!(
        run(&directory, &["read-blob", fields[0]], b"", 0),
        b"include"
    );
    let children = value(&directory, &["read-tree", fields[2]]);
    let marker = value(&directory, &["read-tree", children.lines().next().unwrap()]);
    let fields = marker.lines().collect::<Vec<_>>();
    assert_eq!(
        run(&directory, &["read-blob", fields[0]], b"", 0),
        b"marker.h"
    );
    assert_eq!(
        run(&directory, &["read-blob", fields[2]], b"", 0),
        b"int marker;"
    );
}

#[test]
fn compiler_certificates() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("compiler");
    std::fs::create_dir(&directory).unwrap();
    seed(&directory, &[]);
    let compiler = compiler(&directory);
    let authority = value(&directory, &["resolve", "trusted-compiler"]);
    assert_eq!(value(&directory, &["create-ref", &compiler]), authority);
    assert_eq!(
        std::fs::read_to_string(directory.join(".fix/trust")).unwrap(),
        format!("{authority}\n")
    );
    let closure = value(&directory, &["read-tree", "compile"]);
    let closure: Vec<_> = closure.lines().collect();
    assert_eq!(run(&directory, &["read-blob", closure[0]], b"", 0), b"");
    let fixpoint = value(&directory, &["read-tree", closure[1]]);
    let fixpoint: Vec<_> = fixpoint.lines().collect();
    assert_eq!(fixpoint[..2], [authority.as_str(), compiler.as_str()]);
    assert_eq!(
        run(&directory, &["read-blob", fixpoint[2]], b"", 0),
        b"Runnable"
    );
    let resources = value(&directory, &["read-tree", closure[2]]);
    let resources: Vec<_> = resources.lines().collect();
    assert_eq!(resources[0], authority);
    assert_eq!(
        run(&directory, &["read-blob", resources[2]], b"", 0),
        b"Bootstrap"
    );
    let tools = value(&directory, &["read-tree", resources[1]]);
    let tools: Vec<_> = tools.lines().collect();
    assert_eq!(tools.len(), 3);
    assert_eq!(value(&directory, &["resolve", "wasm2c"]), tools[0]);
    assert_eq!(value(&directory, &["resolve", "clang"]), tools[1]);
    for tool in &tools[..2] {
        let fields = value(&directory, &["read-tree", tool]);
        assert_eq!(fields.lines().next(), Some(authority.as_str()));
    }
    let toolchain = value(&directory, &["read-tree", tools[2]]);
    let name = toolchain.lines().next().unwrap();
    assert_eq!(run(&directory, &["read-blob", name], b"", 0), b"/toolchain");

    let source = value(&directory, &["create-blob", "--string", "source"]);
    let metadata = value(&directory, &["create-blob", "--string", ""]);
    let units = value(&directory, &["create-blob", "--u64", "1"]);
    let combination = value(
        &directory,
        &["create-tree", &metadata, "compile", &source, &units],
    );
    let thunk = value(&directory, &["create-application-thunk", &combination]);
    let next = think(&directory, &thunk);
    assert_eq!(value(&directory, &["get-type", &next]), "thunk");
    let empty = text(run(&directory, &["create-tree", "-"], b"", 0));
    let phase = value(&directory, &["create-blob", "--u64", "3"]);
    let payload = value(
        &directory,
        &["create-tree", "compile", &source, &units, &phase, &empty],
    );
    let claim = value(&directory, &["create-blob", "--string", "Compilation"]);
    let counterfeit = value(&directory, &["create-tree", &authority, &payload, &claim]);
    let application = value(
        &directory,
        &["create-tree", &metadata, "compile", &counterfeit],
    );
    let application = value(&directory, &["create-application-thunk", &application]);
    let result = think(&directory, &application);
    assert_panic(
        &directory,
        &result,
        "assertion failed: tag.is_tag()? && tag.len()? == 3",
    );
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
    let metadata = value(&directory, &["create-blob", "--string", ""]);
    let reply = value(&directory, &["create-blob", "--string", "reply"]);
    let combination = value(&directory, &["create-tree", &metadata, "identity", &reply]);
    let application = value(&directory, &["create-application-thunk", &combination]);
    let config = directory.join("config");
    std::fs::create_dir_all(config.join("fix")).unwrap();
    let request = |mode: &str, input: &str| {
        let mode = value(&directory, &["create-blob", "--string", mode]);
        value(
            &directory,
            &["create-tree", &mode, "evaluator", "coupon-collector", input],
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
            .env("XDG_CONFIG_HOME", &config);
        if let Some(trust) = trust {
            command.env("FIX_TRUST", trust);
        }
        command.output().unwrap()
    };
    let think = request("think", &application);
    let denied = schedule(&directory, "--memoize=never", &think, None);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("untrusted author"));
    std::fs::copy(trust_file(), config.join("fix/trust")).unwrap();
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
fn scheduler_reuses_current_collector_coupons() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("collector_cache");
    let source = directory.join("source");
    std::fs::create_dir_all(&source).unwrap();
    seed(
        &source,
        &[
            ("identity", env!("FIX_IDENTITY")),
            ("tagger", env!("FIX_TAGGER")),
        ],
    );
    let zero = value(&source, &["create-blob", "--u64", "0"]);
    let combination = value(&source, &["create-tree", &zero, "identity", &zero]);
    let thunk = value(&source, &["create-application-thunk", &combination]);
    let strict = value(&source, &["create-strict-encode", &thunk]);
    let shallow = value(&source, &["create-shallow-encode", &thunk]);
    let reference = value(&source, &["create-ref", &combination]);
    assert_eq!(
        evaluate(
            &source,
            "eval",
            "evaluator",
            &strict,
            &["--memoize=always"],
            0
        ),
        combination
    );
    let identity = value(&source, &["read-tree", "identity"]);
    let executable = identity.lines().nth(1).unwrap();
    let foreign_author = value(&source, &["resolve", "tagger"]);
    for (statement, lhs, mode, input, expected) in [
        (1, &strict, "eval", &strict, &combination),
        (3, &thunk, "eval", &shallow, &reference),
        (4, &thunk, "think", &thunk, &combination),
    ] {
        let code = value(&source, &["create-blob", "--u64", &statement.to_string()]);
        let coupons = stored_claims(&source, &[&code, lhs, &combination]);
        let coupon = coupons.first().expect("saved collector coupon");
        let claim_tree = value(&source, &["create-tree", &code, lhs, &combination]);
        let claim_ref = value(&source, &["create-ref", &claim_tree]);
        let fields = value(&source, &["create-tree", &foreign_author, &claim_ref]);
        let request = value(&source, &["create-tree", &zero, "tagger", &fields, &zero]);
        let foreign = value(
            &source,
            &["schedule", "--trusted-tagger", "tagger", &request],
        );
        let archive_path = directory.join(format!("{statement}.tar"));
        let foreign_archive = directory.join(format!("{statement}-foreign.tar"));
        archive(&source, &[input, coupon], &archive_path);
        archive(&source, &[&foreign], &foreign_archive);
        let imported = directory.join(statement.to_string());
        std::fs::create_dir(&imported).unwrap();
        run(&imported, &["init"], b"", 0);
        run(
            &imported,
            &["import", foreign_archive.to_str().unwrap()],
            b"",
            0,
        );
        seed(&imported, &[]);
        std::fs::remove_file(imported.join(".fix/objects").join(executable)).unwrap();
        evaluate(
            &imported,
            mode,
            "evaluator",
            input,
            &["--memoize=always"],
            2,
        );
        run(
            &imported,
            &["import", archive_path.to_str().unwrap()],
            b"",
            0,
        );
        std::fs::remove_file(imported.join(".fix/objects").join(executable)).unwrap();
        evaluate(&imported, mode, "evaluator", input, &["--memoize=never"], 2);
        assert_eq!(
            evaluate(
                &imported,
                mode,
                "evaluator",
                input,
                &["--memoize=always"],
                0
            ),
            *expected
        );
    }
}

#[test]
fn scheduler_modes_dispatch_and_validate_requests() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("scheduler_modes");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(&directory, &[("identity", env!("FIX_IDENTITY"))]);
    let payload = value(&directory, &["create-blob", "--string", "payload"]);
    let combination = value(&directory, &["create-tree", &payload, "identity", &payload]);
    let thunk = value(&directory, &["create-application-thunk", &combination]);
    for (mode, code, input, expected) in [
        ("eval", "1", &payload, &payload),
        ("think", "4", &thunk, &combination),
    ] {
        let mode = value(&directory, &["create-blob", "--string", mode]);
        let request = value(
            &directory,
            &["create-tree", &mode, "evaluator", "coupon-collector", input],
        );
        let coupon = value(
            &directory,
            &[
                "-j",
                "2",
                "schedule",
                "--trusted-tagger",
                "coupon-collector",
                &request,
            ],
        );
        let claim = claim(&directory, &coupon);
        assert_eq!(value(&directory, &["read-blob", "--u64", &claim[0]]), code);
        assert_eq!(&claim[1], input);
        assert_eq!(&claim[2], expected);
    }
    let unknown = value(&directory, &["create-blob", "--string", "unknown"]);
    let tree = value(&directory, &["create-tree", &payload]);
    for mode in [&unknown, &tree] {
        let request = value(
            &directory,
            &[
                "create-tree",
                mode,
                "evaluator",
                "coupon-collector",
                &payload,
            ],
        );
        let result = value(&directory, &["-j", "2", "schedule", &request]);
        assert_panic(&directory, &result, "Fix procedure failed: WrongType");
    }
}

#[test]
fn persistent_plumbing() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("plumbing");
    std::fs::create_dir(&directory).unwrap();
    for arguments in [&[][..], &["--help"], &["-C", ".", "--help"]] {
        let help = value(&directory, arguments);
        assert!(help.to_lowercase().contains("usage:"));
    }
    run(&directory, &["--jobs", "invalid", "eval", "1u64"], b"", 2);
    run(&directory, &["--memoize=sometimes", "repository"], b"", 2);
    run(&directory, &["unknown-command"], b"", 2);
    run(&directory, &["create-blob"], b"", 2);
    run(&directory, &["init"], b"", 0);
    run(&directory, &["init"], b"", 0);
    assert!(run(&directory, &["label", "list"], b"", 0).is_empty());

    for length in [0, 30, 31, 131_073] {
        let bytes: Vec<u8> = (0..length).map(|index| index as u8).collect();
        let blob = text(run(&directory, &["create-blob", "-"], &bytes, 0));
        std::fs::write(directory.join("input.bin"), &bytes).unwrap();
        assert_eq!(value(&directory, &["create-blob", "input.bin"]), blob);
        assert_eq!(blob.len(), 64);
        assert_eq!(run(&directory, &["read-blob", &blob], b"", 0), bytes);
        assert_eq!(
            text(run(&directory, &["create-blob", "-"], &bytes, 0)),
            blob
        );
        assert_eq!(value(&directory, &["len", &blob]), length.to_string());
        if length <= 30 {
            assert!(!directory.join(".fix/objects").join(&blob).exists());
        }
    }

    let string = value(
        &directory,
        &["create-blob", "--string", "-l", "-l", "string"],
    );
    assert_eq!(
        run(&directory, &["read-blob", "--string", "string"], b"", 0),
        b"-l"
    );
    run(&directory, &["label", "set", "second", &string], b"", 0);
    assert_eq!(value(&directory, &["label", "list"]), "second\nstring");
    assert_eq!(value(&directory, &["label", "get", "second"]), string);
    assert_eq!(
        run(
            &directory,
            &["read-blob", "-"],
            format!("{string}\n").as_bytes(),
            0
        ),
        b"-l"
    );
    assert_eq!(value(&directory, &["resolve", "string"]), string);
    for (format, number, width) in [
        ("--u8", "255", 1),
        ("--u32", "4294967295", 4),
        ("--u64", "18446744073709551615", 8),
    ] {
        let blob = value(&directory, &["create-blob", format, number]);
        assert_eq!(
            run(&directory, &["read-blob", &blob], b"", 0),
            vec![255; width]
        );
        assert_eq!(value(&directory, &["read-blob", format, &blob]), number);
    }
    for (format, value) in [
        ("--u8", "256"),
        ("--u32", "4294967296"),
        ("--u64", "18446744073709551616"),
        ("--u64", "-1"),
    ] {
        run(&directory, &["create-blob", format, value], b"", 2);
    }
    run(&directory, &["read-blob", "--u64", "string"], b"", 2);

    std::fs::write(directory.join("first\nfile"), b"binary\0\xff").unwrap();
    std::fs::write(directory.join("second"), b"second").unwrap();
    let imported = text(run(
        &directory,
        &["create-blob", "--files-from", "-"],
        b"first\nfile\0second\0",
        0,
    ));
    let blobs: Vec<_> = imported.lines().collect();
    assert_eq!(blobs.len(), 2);
    assert_eq!(
        run(&directory, &["read-blob", blobs[0]], b"", 0),
        b"binary\0\xff"
    );
    let shared = text(run(&directory, &["create-blob", "-"], &[7; 64], 0));
    let shared_ref = value(&directory, &["create-ref", &shared]);
    let root = value(
        &directory,
        &["-l", "exported", "create-tree", &shared_ref, &shared_ref],
    );
    let closure = value(&directory, &["visit", "exported"]);
    assert_eq!(
        closure.lines().collect::<std::collections::BTreeSet<_>>(),
        [root.as_str(), shared.as_str()].into_iter().collect()
    );
    let blob = text(run(
        &directory,
        &["create-blob", "-", "-l", "input"],
        b"leaf",
        0,
    ));
    let tree = value(&directory, &["create-tree", "input", &blob, "-l", "inputs"]);
    assert_eq!(
        value(&directory, &["read-tree", "inputs"]),
        format!("{blob}\n{blob}")
    );
    run(
        &directory,
        &["label", "set", "alias with space", &blob],
        b"",
        0,
    );
    let unnamed = value(&directory, &["create-blob", "--string", "unnamed"]);
    let mixed = value(&directory, &["create-tree", &blob, &unnamed]);
    assert_eq!(
        value(&directory, &["read-tree", "--labels", &mixed]),
        format!("{blob}\t\"alias with space\"\t\"input\"\n{unnamed}")
    );
    assert_eq!(
        text(run(
            &directory,
            &["create-tree", "-"],
            format!("{blob}\n{blob}\n").as_bytes(),
            0
        )),
        tree
    );
    assert_eq!(value(&directory, &["get-type", "inputs"]), "object");
    assert_eq!(value(&directory, &["get-data-type", "inputs"]), "tree");
    assert_eq!(value(&directory, &["len", "inputs"]), "2");
    run(&directory, &["is-tag", "inputs"], b"", 1);
    run(&directory, &["is-comparable", "inputs"], b"", 0);
    let reference = value(&directory, &["create-ref", "inputs"]);
    assert_eq!(value(&directory, &["get-type", &reference]), "ref");
    assert_eq!(value(&directory, &["len", &reference]), "2");
    run(&directory, &["read-tree", &reference], b"", 2);

    let mut children = vec![tree.clone(), reference.clone()];
    for command in [
        "create-identification-thunk",
        "create-application-thunk",
        "create-selection-thunk",
    ] {
        let thunk = value(&directory, &[command, &reference]);
        assert_eq!(value(&directory, &[command, &tree]), thunk);
        assert_eq!(value(&directory, &["get-type", &thunk]), "thunk");
        run(&directory, &["is-comparable", &thunk], b"", 1);
        run(&directory, &["len", &thunk], b"", 2);
        children.push(thunk.clone());
        for kind in ["strict", "shallow"] {
            let encode = text(run(
                &directory,
                &[&format!("create-{kind}-encode"), "-"],
                format!("{thunk}\n").as_bytes(),
                0,
            ));
            assert_eq!(value(&directory, &["get-encode-type", &encode]), kind);
            run(&directory, &["len", &encode], b"", 2);
            run(&directory, &["label", "set", "pending", &encode], b"", 0);
            assert_eq!(value(&directory, &["resolve", "pending"]), encode);
            children.push(encode);
        }
    }
    let arguments: Vec<&str> = std::iter::once("create-tree")
        .chain(children.iter().map(String::as_str))
        .collect();
    let pending_tree = value(&directory, &arguments);
    assert_eq!(
        value(&directory, &["read-tree", &pending_tree]),
        children.join("\n")
    );
    run(&directory, &["is-comparable", &pending_tree], b"", 1);
    run(&directory, &["create-application-thunk", &blob], b"", 2);
    run(&directory, &["get-data-type", &children[2]], b"", 2);
    run(&directory, &["label", "set", "../outside", &blob], b"", 2);

    let payload = std::fs::read(directory.join(".fix/objects").join(&tree)).unwrap();
    let key = format!("{}ffff", &tree[..60]);
    assert_eq!(
        std::fs::read_link(directory.join(".fix/objects").join(&tree)).unwrap(),
        PathBuf::from(format!("../data/{key}"))
    );
    let mut tagged = tree.clone().into_bytes();
    tagged[62] = b'e';
    let tagged = String::from_utf8(tagged).unwrap();
    run(&directory, &["resolve", &tagged], b"", 2);
    let mut as_blob = tree.clone().into_bytes();
    as_blob[62] = b'8';
    let as_blob = String::from_utf8(as_blob).unwrap();
    run(&directory, &["resolve", &as_blob], b"", 2);
    assert_eq!(
        text(run(&directory, &["create-blob", "-"], &payload, 0)),
        as_blob
    );
    assert_eq!(
        std::fs::read_link(directory.join(".fix/objects").join(&as_blob)).unwrap(),
        PathBuf::from(format!("../data/{key}"))
    );
    assert_eq!(run(&directory, &["read-blob", &as_blob], b"", 0), payload);
    run(&directory, &["resolve", &tagged], b"", 2);

    std::fs::remove_file(directory.join(".fix/objects").join(&as_blob)).unwrap();
    run(&directory, &["resolve", &as_blob], b"", 2);
    let data = directory.join(".fix/data").join(&key);
    let saved = directory.join("payload");
    std::fs::rename(&data, &saved).unwrap();
    std::fs::create_dir(&data).unwrap();
    run(&directory, &["read-tree", &tree], b"", 2);
    std::fs::remove_dir(&data).unwrap();
    std::fs::rename(&saved, &data).unwrap();

    let record = directory.join(".fix/objects").join(&tree);
    std::fs::remove_file(&record).unwrap();
    std::os::unix::fs::symlink("../data/missing", &record).unwrap();
    run(&directory, &["resolve", &tree], b"", 2);
    std::fs::remove_file(&record).unwrap();
    std::os::unix::fs::symlink(format!("../data/{key}"), &record).unwrap();
    std::fs::write(directory.join(".fix/data").join(&key), b"short").unwrap();
    run(&directory, &["read-tree", &tree], b"", 2);

    std::fs::remove_dir_all(directory.join(".fix/labels")).unwrap();
    std::fs::create_dir(directory.join(".fix/labels")).unwrap();
    std::fs::create_dir(directory.join(".fix/labels/blocked")).unwrap();
    run(&directory, &["label", "set", "blocked", &blob], b"", 2);
}

#[test]
fn repository_discovery() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("discovery");
    let nested = directory.join("a/b");
    let other = directory.join("other");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(&other).unwrap();
    run(&directory, &["init"], b"", 0);
    assert_eq!(
        value(&nested, &["repository"]),
        directory.join(".fix").to_str().unwrap()
    );
    let root_blob = text(run(
        &nested,
        &["create-blob", "-", "-l", "input"],
        &[1; 64],
        0,
    ));
    assert!(!nested.join(".fix").exists());
    assert_eq!(run(&directory, &["read-blob", "input"], b"", 0), [1; 64]);

    run(&directory, &["-C", "other", "init"], b"", 0);
    assert_eq!(
        value(&nested, &["-C", "../../other", "repository"]),
        other.join(".fix").to_str().unwrap()
    );
    let other_blob = text(run(
        &directory,
        &["-C", "other", "create-blob", "-", "-l", "input"],
        &[2; 64],
        0,
    ));
    assert_ne!(other_blob, root_blob);
    assert_eq!(
        run(
            &nested,
            &["-C", "../../other", "read-blob", "input"],
            b"",
            0
        ),
        [2; 64]
    );
    assert_eq!(
        value(
            &nested,
            &["-C", directory.to_str().unwrap(), "resolve", "input"]
        ),
        root_blob
    );
    assert_eq!(
        value(
            &directory,
            &["-C", "other", "-C", "../a/b", "resolve", "input"]
        ),
        root_blob
    );
    run(&directory, &["-C"], b"", 2);
    run(&directory, &["-C", "missing", "resolve", "input"], b"", 2);

    run(&nested, &["init"], b"", 0);
    let local_blob = text(run(
        &nested,
        &["create-blob", "-", "-l", "input"],
        &[3; 64],
        0,
    ));
    assert_eq!(run(&nested, &["read-blob", "input"], b"", 0), [3; 64]);
    assert_ne!(local_blob, root_blob);
    run(&nested, &["read-blob", &root_blob], b"", 2);
    std::os::unix::fs::symlink("a/b", directory.join("alias")).unwrap();
    assert_eq!(
        value(&directory, &["-C", "alias", "resolve", "input"]),
        local_blob
    );

    std::fs::remove_dir_all(nested.join(".fix")).unwrap();
    std::fs::write(nested.join(".fix"), b"invalid repository").unwrap();
    run(&nested, &["read-blob", "input"], b"", 2);
    std::fs::remove_file(nested.join(".fix")).unwrap();
    std::fs::create_dir(nested.join(".fix")).unwrap();
    run(&nested, &["create-blob", "-", "-l", "input"], &[4; 64], 2);
    assert_eq!(value(&directory, &["resolve", "input"]), root_blob);
}

#[test]
fn evaluation_modes() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("evaluation");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(&directory, &[]);
    let tree = |handles: &[&str]| {
        let arguments: Vec<_> = std::iter::once("create-tree")
            .chain(handles.iter().copied())
            .collect();
        value(&directory, &arguments)
    };
    let encode =
        |kind: &str, thunk: &str| value(&directory, &[&format!("create-{kind}-encode"), thunk]);
    let blob = text(run(&directory, &["create-blob", "-"], &[42; 64], 0));
    let reference = value(&directory, &["create-ref", &blob]);
    let identity = value(&directory, &["create-identification-thunk", &reference]);
    let contents = tree(&[&reference, &encode("strict", &identity)]);
    let contents_ref = value(&directory, &["create-ref", &contents]);
    let contents_identity = value(&directory, &["create-identification-thunk", &contents_ref]);
    let evaluated = tree(&[&reference, &blob]);
    let index = value(&directory, &["create-blob", "--u64", "0"]);
    let select = |handle: &str| {
        let container = tree(&[handle]);
        let container_ref = value(&directory, &["create-ref", &container]);
        let selection = tree(&[&container_ref, &index]);
        value(&directory, &["create-selection-thunk", &selection])
    };
    let returned_strict = select(&encode("strict", &contents_identity));
    let returned_shallow = select(&encode("shallow", &contents_identity));
    let cases = [
        (reference.clone(), reference),
        (contents_ref.clone(), contents_ref.clone()),
        (contents, evaluated.clone()),
        (encode("strict", &contents_identity), evaluated.clone()),
        (encode("shallow", &contents_identity), contents_ref.clone()),
        (encode("shallow", &returned_strict), contents_ref),
        (encode("strict", &returned_shallow), evaluated.clone()),
    ];
    for jobs in ["1", "2"] {
        for (input, expected) in &cases {
            assert_eq!(
                evaluate(&directory, "eval", "evaluator", input, &["-j", jobs], 0),
                *expected,
                "jobs={jobs}, input={input}"
            );
        }
    }
    for width in [0, 4, 16] {
        let index = text(run(&directory, &["create-blob", "-"], &vec![0; width], 0));
        let arguments = tree(&[&evaluated, &index]);
        let thunk = value(&directory, &["create-selection-thunk", &arguments]);
        let selection = encode("strict", &thunk);
        for jobs in ["1", "2"] {
            let result = evaluate_raw(
                &directory,
                "eval",
                "evaluator",
                &selection,
                &["-j", jobs],
                0,
            );
            assert_panic(
                &directory,
                &result,
                "assertion `left == right` failed: scheduler effect failed\n  left: 5\n right: 0",
            );
        }
    }
}

#[test]
fn tag_authors_are_checked_as_objects_or_references() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("checked-authors");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(&directory, &[("tagger", env!("FIX_TAGGER"))]);
    let zero = value(&directory, &["create-blob", "--u64", "0"]);
    let mode = value(&directory, &["create-blob", "--u64", "7"]);
    let callable = value(&directory, &["resolve", "tagger"]);
    let reference = value(&directory, &["create-ref", &callable]);
    for author in [&callable, &reference] {
        let fields = value(&directory, &["create-tree", author, &zero]);
        let request = value(
            &directory,
            &["create-tree", &zero, &callable, &fields, &mode],
        );
        let tag = value(&directory, &["-j", "1", "schedule", &request]);
        run(&directory, &["is-tag", &tag], b"", 0);
        assert_eq!(
            value(&directory, &["read-tree", &tag]),
            format!("{author}\n{zero}")
        );
    }
    let compiler = compiler(&directory);
    let authority = value(&directory, &["resolve", "trusted-compiler"]);
    let thunk = value(&directory, &["create-identification-thunk", &zero]);
    let opaque = value(&directory, &["create-tree", &thunk]);
    for author in [&compiler, &authority] {
        let fields = value(&directory, &["create-tree", author, &zero]);
        let request = value(
            &directory,
            &["create-tree", &zero, &callable, &fields, &mode],
        );
        let result = value(&directory, &["-j", "1", "schedule", &request]);
        assert_trap(&directory, &result, 5);
    }
    let fields = value(&directory, &["create-tree", &opaque, &zero]);
    let request = value(
        &directory,
        &["create-tree", &zero, &callable, &fields, &mode],
    );
    let result = value(&directory, &["-j", "1", "schedule", &request]);
    assert_panic(&directory, &result, "Fix procedure failed: NotComparable");
}

#[test]
fn trusted_tagger_authority_is_separate_from_scheduler_privilege() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("taggers");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(
        &directory,
        &[
            ("tagger", env!("FIX_TAGGER")),
            ("identity", env!("FIX_IDENTITY")),
        ],
    );
    let empty = value(&directory, &["create-blob", "--string", ""]);
    let zero = value(&directory, &["create-blob", "--u64", "0"]);
    let authors = value(&directory, &["create-tree", "-"]);
    let fields = value(&directory, &["create-tree", &authors, &zero]);
    let byte_query = value(
        &directory,
        &[
            "create-tree",
            &empty,
            "tagger",
            &fields,
            &value(&directory, &["create-blob", "--u64", "8"]),
        ],
    );
    assert_eq!(
        value(&directory, &["-j", "1", "schedule", &byte_query]),
        fields
    );
    let byte_thunk = value(&directory, &["create-application-thunk", &byte_query]);
    assert_trap(&directory, &think(&directory, &byte_thunk), 5);
    let kind_query = value(
        &directory,
        &[
            "create-tree",
            &empty,
            "tagger",
            &fields,
            &value(&directory, &["create-blob", "--u64", "6"]),
        ],
    );
    assert_eq!(
        value(&directory, &["-j", "1", "schedule", &kind_query]),
        fields
    );
    let thunk = value(&directory, &["create-application-thunk", &kind_query]);
    assert_trap(&directory, &think(&directory, &thunk), 5);
    let combination = value(
        &directory,
        &["create-tree", &empty, "tagger", &fields, &zero],
    );
    assert_trap(
        &directory,
        &value(&directory, &["-j", "1", "schedule", &combination]),
        5,
    );
    let denied = value(
        &directory,
        &[
            "-j",
            "1",
            "schedule",
            "--trusted-tagger",
            "identity",
            &combination,
        ],
    );
    assert_trap(&directory, &denied, 5);
    let result = value(
        &directory,
        &[
            "-j",
            "1",
            "schedule",
            "--trusted-tagger",
            "tagger",
            &combination,
        ],
    );
    run(&directory, &["is-tag", &result], b"", 0);
    let output = value(&directory, &["read-tree", &result]);
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        [authors.as_str(), zero.as_str()]
    );
    assert!(value(&directory, &["read-tree", &authors]).is_empty());

    let closure = value(&directory, &["create-tree", &empty, "tagger", "identity"]);
    let indirect = value(
        &directory,
        &["create-tree", &empty, &closure, &fields, &zero],
    );
    let denied = value(
        &directory,
        &[
            "-j",
            "1",
            "schedule",
            "--trusted-tagger",
            "identity",
            &indirect,
        ],
    );
    assert_trap(&directory, &denied, 5);

    let one = value(&directory, &["create-blob", "--u64", "1"]);
    let paused = value(
        &directory,
        &["create-tree", &empty, "tagger", &fields, &one, &empty],
    );
    let thunk = value(&directory, &["create-application-thunk", &paused]);
    let request = think(&directory, &thunk);
    let pair = value(&directory, &["read-tree", &request]);
    let continuation = pair.lines().nth(1).unwrap();
    let resumed = value(&directory, &["create-tree", &empty, continuation, &empty]);
    for grants in [vec![], vec!["--trusted-tagger", "identity"]] {
        let mut command = vec!["-j", "1", "schedule"];
        command.extend(grants);
        command.push(&resumed);
        assert_trap(&directory, &value(&directory, &command), 5);
    }
    assert_eq!(
        value(
            &directory,
            &[
                "-j",
                "1",
                "schedule",
                "--trusted-tagger",
                "tagger",
                &resumed
            ],
        ),
        result
    );
}

#[test]
fn collector_tag_reconstruction_rejects_scheduler_assumptions() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("collector_tags");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(
        &directory,
        &[
            ("collector", env!("FIX_COLLECTOR")),
            ("scheduler", env!("FIX_TAGGER")),
        ],
    );
    let blob = |n: u64| value(&directory, &["create-blob", "--u64", &n.to_string()]);
    let tree = |fields: &[&str]| {
        let mut args = vec!["create-tree"];
        args.extend_from_slice(fields);
        if fields.is_empty() {
            args.push("-");
        }
        value(&directory, &args)
    };
    let empty = tree(&[]);
    let zero = blob(0);
    let one = blob(1);
    let tag = |fields: &str, trusted: bool| {
        let mode = blob(if trusted { 0 } else { 2 });
        let combination = tree(&[&zero, "scheduler", fields, &mode]);
        let mut args = vec!["-j", "1", "schedule"];
        if trusted {
            args.extend(["--trusted-tagger", "scheduler"]);
        }
        args.push(&combination);
        value(&directory, &args)
    };
    let request = |rule: u64, lhs: &str, rhs: &str, witnesses: &[&str], rejected| {
        let rule = blob(rule);
        let witnesses = tree(witnesses);
        let combination = tree(&[&zero, "collector", &rule, lhs, rhs, &witnesses]);
        let scheduler = tree(&[&zero, "scheduler", &combination, &blob(3)]);
        let result = value(
            &directory,
            &[
                "-j",
                "1",
                "schedule",
                "--trusted-tagger",
                "collector",
                &scheduler,
            ],
        );
        if rejected {
            assert_trap(&directory, &result, 5);
        }
        result
    };
    let witness = |coupon: &str, kind: u64, lhs: &str, rhs: &str| {
        let claim = tree(&[&blob(kind), lhs, rhs]);
        tree(&[coupon, &claim])
    };
    let fields = tree(&[&empty, &zero]);
    let original = tag(&fields, true);
    let authors_coupon = request(5, &empty, &empty, &[], false);
    let authors = witness(&authors_coupon, 1, &empty, &empty);
    let leaf_coupon = request(5, &zero, &zero, &[], false);
    let leaf = witness(&leaf_coupon, 1, &zero, &zero);
    assert_eq!(
        claim(
            &directory,
            &request(5, &original, &fields, &[&authors, &leaf], false)
        )[2],
        original
    );
    let ordinary = request(5, &fields, &fields, &[&authors, &leaf], false);
    assert_eq!(claim(&directory, &ordinary)[2], fields);
    let reference = value(&directory, &["create-ref", &zero]);
    let thunk = value(&directory, &["create-identification-thunk", &reference]);
    let encoded = value(&directory, &["create-strict-encode", &thunk]);
    let thought = witness(
        &request(2, &thunk, &reference, &[], false),
        4,
        &thunk,
        &reference,
    );
    let forced = witness(
        &request(3, &thunk, &reference, &[&thought], false),
        3,
        &thunk,
        &reference,
    );
    let eq = witness(
        &request(7, &encoded, &zero, &[&forced], false),
        0,
        &encoded,
        &zero,
    );
    let reversed = witness(
        &request(10, &zero, &encoded, &[&eq], false),
        0,
        &zero,
        &encoded,
    );
    let payload = witness(
        &request(8, &encoded, &zero, &[&leaf, &reversed], false),
        1,
        &encoded,
        &zero,
    );
    let unevaluated = tag(&tree(&[&empty, &encoded]), true);
    assert_eq!(
        claim(
            &directory,
            &request(5, &unevaluated, &fields, &[&authors, &payload], false)
        )[2],
        original
    );
    let changed = tree(&[&empty, &one]);
    let invalid = tree(&[&zero, &zero]);
    for (lhs, rhs, proofs) in [
        (
            original.as_str(),
            changed.as_str(),
            vec![authors.as_str(), leaf.as_str()],
        ),
        (
            original.as_str(),
            fields.as_str(),
            vec![leaf.as_str(), authors.as_str()],
        ),
        (original.as_str(), fields.as_str(), vec![authors.as_str()]),
        (
            original.as_str(),
            invalid.as_str(),
            vec![leaf.as_str(), leaf.as_str()],
        ),
    ] {
        request(5, lhs, rhs, &proofs, true);
    }
    let fake_claim = tree(&[&one, &zero, &one]);
    let fake_ref = value(&directory, &["create-ref", &fake_claim]);
    let fake_coupon = tag(&tree(&[&empty, &fake_ref]), false);
    let fake = tree(&[&fake_coupon, &fake_claim]);
    request(5, &original, &changed, &[&authors, &fake], true);
    let candidate = tag(&changed, true);
    request(5, &original, &candidate, &[&authors, &fake], true);
}

#[test]
fn scheduler_effects_repository_round_trip() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("user_scheduler");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(
        &directory,
        &[
            ("scheduler", env!("USER_SCHEDULER")),
            ("identity", env!("FIX_IDENTITY")),
            ("continuation", env!("FIX_CONTINUATION")),
        ],
    );
    let bytes = [42; 64];
    let blob = text(run(&directory, &["create-blob", "-"], &bytes, 0));
    let metadata = value(&directory, &["create-blob", "--string", ""]);
    let root = value(
        &directory,
        &["create-tree", &metadata, "identity", &blob, "continuation"],
    );
    let index = value(&directory, &["create-blob", "--u64", "2"]);
    let selection = value(&directory, &["create-tree", &root, &index]);
    let combination = value(
        &directory,
        &["create-tree", &metadata, "scheduler", &metadata, &root],
    );
    for workers in ["1", "2"] {
        let result = value(&directory, &["-j", workers, "schedule", &combination]);
        let fields = value(&directory, &["read-tree", &result]);
        let fields: Vec<_> = fields.lines().collect();
        assert_eq!(fields.len(), 6);
        for (coupon, operation, input, output) in [
            (fields[0], 2, root.as_str(), root.as_str()),
            (fields[1], 6, selection.as_str(), blob.as_str()),
            (fields[2], 0, root.as_str(), root.as_str()),
            (fields[5], 1, root.as_str(), root.as_str()),
        ] {
            let claim = claim(&directory, coupon);
            assert_eq!(
                value(&directory, &["read-blob", "--u64", &claim[0]]),
                operation.to_string()
            );
            assert_eq!(claim[1], input);
            assert_eq!(claim[2], output);
        }
        assert_eq!(fields[3], root);
        assert_eq!(fields[4], root);
        assert_eq!(run(&directory, &["read-blob", &blob], b"", 0), bytes);
    }

    let scheduler = value(&directory, &["resolve", "scheduler"]);
    let combination = value(
        &directory,
        &["create-tree", &metadata, &scheduler, &metadata, &root],
    );
    let thunk = value(&directory, &["create-application-thunk", &combination]);
    let request = think(&directory, &thunk);
    let pair = value(&directory, &["read-tree", &request]);
    let pair: Vec<_> = pair.lines().collect();
    assert_eq!(pair.len(), 2);
    let effect = value(&directory, &["read-tree", pair[0]]);
    let name = effect.lines().next().unwrap();
    assert_eq!(
        text(run(&directory, &["read-blob", name], b"", 0)),
        "worker_config"
    );
    let zero = value(&directory, &["create-blob", "--u64", "0"]);
    let reply = value(&directory, &["create-tree", &zero, &zero]);
    let resumed = value(&directory, &["create-tree", &metadata, pair[1], &reply]);
    let thunk = value(&directory, &["create-application-thunk", &resumed]);
    let request = think(&directory, &thunk);
    let pair = value(&directory, &["read-tree", &request]);
    let effect = value(&directory, &["read-tree", pair.lines().next().unwrap()]);
    assert_eq!(
        effect.lines().nth(1).unwrap(),
        value(&directory, &["create-blob", "--u64", "1"])
    );
}

#[test]
fn persistent_continuation() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("continuation");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(&directory, &[("continuation", env!("FIX_CONTINUATION"))]);
    let program = value(&directory, &["resolve", "continuation"]);
    let resources = value(&directory, &["create-blob", "--string", ""]);
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

#[test]
fn serial_and_parallel_schedulers_evaluate_applications_and_selections() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("scheduler_sdk");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(
        &directory,
        &[
            ("serial", env!("SERIAL_SCHEDULER")),
            ("parallel", env!("FIX_SCHEDULER")),
            ("advisor", env!("SDK_ADVISOR")),
        ],
    );
    let metadata = value(&directory, &["create-blob", "--string", ""]);
    let first = value(&directory, &["create-blob", "--string", "first"]);
    let second = value(&directory, &["create-blob", "--string", "second"]);
    let combination = value(&directory, &["create-tree", &metadata, "advisor", &first]);
    let application = value(&directory, &["create-application-thunk", &combination]);
    let application = value(&directory, &["create-strict-encode", &application]);
    let values = value(&directory, &["create-tree", &first, &second]);
    let index = value(&directory, &["create-blob", "--u64", "1"]);
    let selection = value(&directory, &["create-tree", &values, &index]);
    let selection = value(&directory, &["create-selection-thunk", &selection]);
    let selection = value(&directory, &["create-strict-encode", &selection]);
    let input = value(&directory, &["create-tree", &application, &selection]);
    for (scheduler, workers) in [
        ("serial", "1"),
        ("serial", "4"),
        ("parallel", "1"),
        ("parallel", "4"),
    ] {
        let output = evaluate(
            &directory,
            "eval",
            scheduler,
            &input,
            &["-j", workers, "--memoize=never"],
            0,
        );
        let fields = value(&directory, &["read-tree", &output]);
        assert_eq!(
            fields.lines().collect::<Vec<_>>(),
            [first.as_str(), second.as_str()]
        );
    }
}

#[test]
fn canonicalization_advice_stays_with_the_requested_input() {
    for workers in ["1", "4"] {
        let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap())
            .join(format!("advice_input_{workers}"));
        compiler_fixture(&directory);
        let empty = value(&directory, &["create-blob", "--string", ""]);
        let source = value(&directory, &["create-blob", "--string", "source"]);
        let compilation = compilation(&directory, &source);
        let ordinary = value(
            &directory,
            &["create-tree", &empty, "identity", &compilation],
        );
        let ordinary = value(&directory, &["create-application-thunk", &ordinary]);
        let strict = value(&directory, &["create-strict-encode", &ordinary]);
        let first = value(&directory, &["create-tree", &strict]);
        let options = ["--memoize=auto", "-j", workers];
        let result = evaluate(&directory, "eval", "evaluator", &first, &options, 0);
        let ready = value(&directory, &["read-tree", &result]);
        let fields = value(&directory, &["read-tree", &ready]);
        let runnable = fields.lines().nth(2).unwrap();
        run(&directory, &["is-tag", runnable], b"", 0);
        for statement in ["3", "4"] {
            let statement = value(&directory, &["create-blob", "--u64", statement]);
            assert!(stored_claims(&directory, &[&statement, &ordinary]).is_empty());
        }
        let executable = directory.join(".fix/objects").join(compiler(&directory));
        let backing = std::fs::read_link(&executable).unwrap();
        std::fs::remove_file(executable.parent().unwrap().join(backing)).unwrap();
        let second = value(&directory, &["create-tree", &strict, &empty]);
        let expected = value(&directory, &["create-tree", &ready, &empty]);
        assert_eq!(
            evaluate(&directory, "eval", "evaluator", &second, &options, 0),
            expected
        );
    }
}

#[test]
fn canonicalization_advice_does_not_acquire_unavailable_references() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("unavailable_advice");
    std::fs::create_dir(&directory).unwrap();
    run(&directory, &["init"], b"", 0);
    seed(&directory, &[("advisor", env!("SDK_UNAVAILABLE_ADVISOR"))]);
    let object = text(run(&directory, &["create-blob", "-"], &[b'x'; 64], 0));
    let reference = value(&directory, &["create-ref", &object]);
    let record = directory.join(".fix/objects").join(&object);
    let backing = record
        .parent()
        .unwrap()
        .join(std::fs::read_link(&record).unwrap());
    std::fs::remove_file(&backing).unwrap();
    let empty = value(&directory, &["create-blob", "--string", ""]);
    let combination = value(&directory, &["create-tree", &empty, "advisor", &reference]);
    let application = value(&directory, &["create-application-thunk", &combination]);
    let input = value(&directory, &["create-strict-encode", &application]);
    for workers in ["1", "4"] {
        let result = evaluate(
            &directory,
            "eval",
            "evaluator",
            &input,
            &["--memoize=never", "-j", workers],
            0,
        );
        assert_eq!(run(&directory, &["read-blob", &result], b"", 0), b"ok");
        assert!(!backing.exists());
    }
}

#[test]
fn repl_evaluates_expressions_imports_and_recovers_from_errors() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("repl");
    seed(
        &directory,
        &[
            ("fix-parser", env!("FIX_PARSER")),
            ("procedure-name", env!("FIX_IDENTITY")),
        ],
    );
    std::fs::write(directory.join("input.bin"), b"contents").unwrap();
    let expected = [
        value(&directory, &["42u64"]),
        value(
            &directory,
            &[r#"*#("" @label("procedure-name") @file("input.bin"))"#],
        ),
        value(&directory, &["(1u8 2u8)"]),
        value(&directory, &["7u64"]),
    ];
    let output = run(
        &directory,
        &["-j", "1", "-b", "supplied=7u64", "repl"],
        b"42u64\n@file(\"missing.bin\")\n:{\n*#(\"\" @label(\"procedure-name\")\n@file(\"input.bin\"))\n:}\n(1u8 2u8)\n$supplied\n",
        0,
    );
    assert_eq!(text(output), expected.join("\n"));
}
