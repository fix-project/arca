use fix::{DataType, ValueType};
use fixcli::{Client, Options};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Once,
};

static CONFIGURE_TRUST: Once = Once::new();

fn input(name: &str) -> PathBuf {
    PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap())
        .join(name)
}
fn repository(name: &str) -> PathBuf {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join(name);
    std::fs::create_dir(&directory).unwrap();
    let output = Command::new(input(env!("BACKEND")))
        .args(["init"])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(output.status.success());
    std::fs::create_dir(directory.join("toolchain")).unwrap();
    let output = Command::new(input(env!("SEED")))
        .args(["create", "."])
        .arg(input(env!("COMPILER")))
        .arg(input(env!("ADD")))
        .arg(input(env!("ADD")))
        .arg("toolchain")
        .arg(format!("scheduler={}", input(env!("SCHEDULER")).display()))
        .arg(format!("collector={}", input(env!("COLLECTOR")).display()))
        .arg(format!("add={}", input(env!("ADD")).display()))
        .arg(format!("fix-parser={}", input(env!("PARSER")).display()))
        .arg(format!(
            "scheduler-failure={}",
            input(env!("SCHEDULER_FAILURE")).display()
        ))
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    CONFIGURE_TRUST.call_once(|| {
        let config = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("api-config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("trusted_taggers"),
            format!(
                "{}\n",
                std::fs::read_to_string(directory.join(".fix/labels/collector"))
                    .unwrap()
                    .trim()
            ),
        )
        .unwrap();
        unsafe { std::env::set_var("FIX_CONFIG_DIR", config) };
    });
    directory
}
fn client(directory: &Path) -> Client {
    Client::open(
        directory,
        Options {
            trust: directory
                .join(".fix/trust")
                .exists()
                .then(|| directory.join(".fix/trust")),
            unsigned: true,
            memory_mib: 128,
            ..Options::default()
        },
    )
    .unwrap()
}

#[test]
fn scheduler_failures_report_the_scheduler_and_guest_error() {
    let directory = repository("scheduler-errors");
    for (input, expected) in [
        (0u64, "Rust panic: named input cannot be loaded"),
        (1u64, "unexpected scheduler result"),
    ] {
        let scheduler = "scheduler-failure";
        let mut options = client(&directory).options().clone();
        options.scheduler = scheduler.into();
        let mut client = Client::open(&directory, options).unwrap();
        let program = client.lookup(scheduler).unwrap();
        let program = client.save(&program).unwrap();
        let input = client.blob(&input.to_le_bytes()).unwrap();
        let error = match client.eval(&input) {
            Ok(_) => panic!("expected the malformed scheduler invocation to fail"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains(&program), "{error}");
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn parser_failures_are_distinct_from_literal_error_values() {
    let directory = repository("parser-responses");
    let invoke = |source: &str, config: &Path| {
        Command::new(input(env!("BACKEND")))
            .args(["-j", "1", "--memory", "512", "eval", source])
            .current_dir(&directory)
            .env("FIX_TRUST", directory.join(".fix/trust"))
            .env("FIX_UNSIGNED", "true")
            .env("FIX_CONFIG_DIR", config)
            .output()
            .unwrap()
    };
    let config = PathBuf::from(std::env::var_os("FIX_CONFIG_DIR").unwrap());
    let failed = invoke("(", &config);
    assert!(!failed.status.success());
    let diagnostic = String::from_utf8(failed.stderr).unwrap();
    assert!(
        diagnostic.contains("parser fix-parser failed for"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("unexpected"), "{diagnostic}");
    let output = invoke(r#"("Rust.panic" "valid data")"#, &config);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let name = String::from_utf8(output.stdout).unwrap();
    let mut client = client(&directory);
    let value = client.lookup(name.trim()).unwrap();
    let message = client.get(&value, 1).unwrap();
    let mut bytes = [0; 10];
    client.read(&message, 0, &mut bytes).unwrap();
    assert_eq!(&bytes, b"valid data");

    let untrusted = directory.join("untrusted-config");
    std::fs::create_dir(&untrusted).unwrap();
    let output = invoke(r#""input""#, &untrusted);
    assert_eq!(output.status.code(), Some(2));
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostic.contains("not a trusted tagger"), "{diagnostic}");
    assert!(
        diagnostic.contains("evaluate ") && diagnostic.contains("failed:"),
        "{diagnostic}"
    );
}

#[test]
fn an_empty_repository_uses_user_default_programs() {
    let root = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("user-defaults");
    let user = repository("user-defaults");
    let data = root.join("data");
    std::fs::create_dir_all(data.join("fix/bootstrap")).unwrap();
    std::fs::rename(user.join(".fix"), data.join("fix/bootstrap/.fix")).unwrap();
    let bootstrap_repository =
        fixrepository::Repository::open(data.join("fix/bootstrap/.fix").to_str().unwrap())
            .unwrap()
            .unsigned();
    let scheduler = bootstrap_repository.read_label("scheduler").unwrap();
    bootstrap_repository
        .write_label("serial", scheduler)
        .unwrap();
    let user_repository = fixrepository::Repository::init(user.join(".fix").to_str().unwrap())
        .unwrap()
        .unsigned();
    let mut home_client = client(&user);
    let home_blob = home_client.blob(b"home").unwrap();
    home_client.label("payload", &home_blob).unwrap();
    let home_value = user_repository.read_label("payload").unwrap();
    std::fs::remove_file(data.join("fix/bootstrap/.fix/labels/scheduler")).unwrap();
    let project = root.join("project");
    std::fs::create_dir(&project).unwrap();
    let initialized = Command::new(input(env!("BACKEND")))
        .arg("init")
        .current_dir(&project)
        .env("FIX_CONFIG_DIR", user.join("config"))
        .env("XDG_DATA_HOME", &data)
        .output()
        .unwrap();
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let repository =
        fixrepository::Repository::open(project.join(".fix").to_str().unwrap()).unwrap();
    let remotes = repository.remotes().unwrap();
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].name, "bootstrap");
    assert_eq!(
        remotes[0].address,
        format!("file://{}", data.join("fix/bootstrap/.fix").display())
    );
    for (setting, name) in [
        (
            "scheduler",
            bootstrap_repository
                .read_label("serial")
                .unwrap()
                .to_string(),
        ),
        (
            "collector",
            bootstrap_repository
                .read_label("collector")
                .unwrap()
                .to_string(),
        ),
        (
            "parser",
            bootstrap_repository
                .read_label("fix-parser")
                .unwrap()
                .to_string(),
        ),
        (
            "compiler",
            bootstrap_repository
                .read_label("compile")
                .unwrap()
                .to_string(),
        ),
    ] {
        let configured = Command::new(input(env!("BACKEND")))
            .args(["config", "--global", setting, &name])
            .current_dir(&project)
            .env("FIX_CONFIG_DIR", user.join("config"))
            .env("XDG_DATA_HOME", &data)
            .output()
            .unwrap();
        assert!(configured.status.success());
    }
    std::fs::write(
        user.join("config/trusted_taggers"),
        format!(
            "{}\n",
            bootstrap_repository.read_label("collector").unwrap()
        ),
    )
    .unwrap();
    let output = Command::new(input(env!("BACKEND")))
        .args(["-j", "1", "--memory", "512", "eval", "1u64"])
        .current_dir(&project)
        .env("FIX_CONFIG_DIR", user.join("config"))
        .env("XDG_DATA_HOME", &data)
        .env("FIX_TRUST", data.join("fix/bootstrap/.fix/trust"))
        .env("FIX_UNSIGNED", "true")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let repository = fixrepository::Repository::open(project.join(".fix").to_str().unwrap())
        .unwrap()
        .unsigned();
    assert!(repository.labels().unwrap().is_empty());
    assert!(
        std::fs::read_dir(project.join(".fix/objects"))
            .unwrap()
            .next()
            .is_some()
    );
    let mut project_client = client(&project);
    let project_blob = project_client.blob(b"project").unwrap();
    project_client.label("payload", &project_blob).unwrap();
    let project_value = repository.read_label("payload").unwrap();
    let file_url = format!("file://{}", user.join(".fix").display());
    let added = Command::new(input(env!("BACKEND")))
        .args(["remote", "add", "home", &file_url])
        .current_dir(&project)
        .env("FIX_CONFIG_DIR", user.join("config"))
        .env("XDG_DATA_HOME", &data)
        .output()
        .unwrap();
    assert!(added.status.success());
    let synced = Command::new(input(env!("BACKEND")))
        .args(["sync", "home"])
        .current_dir(&project)
        .env("FIX_CONFIG_DIR", user.join("config"))
        .env("XDG_DATA_HOME", &data)
        .env("FIX_UNSIGNED", "true")
        .output()
        .unwrap();
    assert!(
        synced.status.success(),
        "{}",
        String::from_utf8_lossy(&synced.stderr)
    );
    let replacement = home_client.blob(b"new home").unwrap();
    home_client.label("payload", &replacement).unwrap();
    let pulled = Command::new(input(env!("BACKEND")))
        .args(["pull", "home:payload", "from-home"])
        .current_dir(&project)
        .env("FIX_CONFIG_DIR", user.join("config"))
        .env("XDG_DATA_HOME", &data)
        .env("FIX_UNSIGNED", "true")
        .output()
        .unwrap();
    assert!(
        pulled.status.success(),
        "{}",
        String::from_utf8_lossy(&pulled.stderr)
    );
    assert_eq!(
        repository.read_label("payload").unwrap().as_bytes(),
        project_value.as_bytes()
    );
    assert_eq!(
        repository.read_label("from-home").unwrap().as_bytes(),
        home_value.as_bytes()
    );
    let labels = Command::new(input(env!("BACKEND")))
        .args(["remote", "labels", "home"])
        .current_dir(&project)
        .env("FIX_CONFIG_DIR", user.join("config"))
        .env("XDG_DATA_HOME", &data)
        .env("FIX_UNSIGNED", "true")
        .output()
        .unwrap();
    assert!(labels.status.success());
    assert!(
        String::from_utf8(labels.stdout)
            .unwrap()
            .contains(&format!("home:payload\t{home_value}"))
    );
    let pushed = Command::new(input(env!("BACKEND")))
        .args(["push", "payload", "home:project-copy"])
        .current_dir(&project)
        .env("FIX_CONFIG_DIR", user.join("config"))
        .env("XDG_DATA_HOME", &data)
        .env("FIX_UNSIGNED", "true")
        .output()
        .unwrap();
    assert!(
        pushed.status.success(),
        "{}",
        String::from_utf8_lossy(&pushed.stderr)
    );
    assert_eq!(
        user_repository
            .read_label("project-copy")
            .unwrap()
            .as_bytes(),
        project_value.as_bytes()
    );
}

#[test]
fn local_values_can_be_read_without_an_evaluator_and_saved() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("local-values");
    fixrepository::Repository::init(directory.join(".fix").to_str().unwrap()).unwrap();
    let mut client = Client::open(&directory, Options::default()).unwrap();
    let contents: Vec<u8> = (0..128).collect();
    let blob = client.blob(&contents).unwrap();
    let number = client.blob(&42u64.to_le_bytes()).unwrap();
    let tree = client.tree(&[blob.clone(), number]).unwrap();
    assert_eq!(client.value_type(&tree).unwrap(), ValueType::Object);
    assert_eq!(client.data_type(&tree).unwrap(), DataType::Tree);
    assert_eq!(client.len(&tree).unwrap(), 2);
    assert!(!client.is_tag(&tree).unwrap());
    assert!(client.is_comparable(&tree).unwrap());
    let reference = client.reference(&tree).unwrap();
    assert_eq!(client.value_type(&reference).unwrap(), ValueType::Ref);
    let child = client.get(&tree, 0).unwrap();
    assert!(client.equals(&child, &blob).unwrap());
    assert_eq!(client.data_type(&child).unwrap(), DataType::Blob);
    assert_eq!(client.len(&child).unwrap(), contents.len() as u64);
    let mut slice = [0; 17];
    client.read(&child, 31, &mut slice).unwrap();
    assert_eq!(slice, contents[31..48]);
    let name = client.save(&tree).unwrap();
    drop(tree);
    drop(blob);
    client.read(&child, 31, &mut slice).unwrap();
    assert_eq!(slice, contents[31..48]);
    client.close().unwrap();

    let mut client = Client::open(&directory, Options::default()).unwrap();
    let restored = client.lookup(&name).unwrap();
    let child = client.get(&restored, 0).unwrap();
    let mut result = vec![0; contents.len()];
    client.read(&child, 0, &mut result).unwrap();
    assert_eq!(result, contents);
}

#[test]
fn persistent_session_values_evaluation_and_storage() {
    let directory = repository("roundtrip");
    let mut session = client(&directory);
    session.reuse_vm();
    let left = session.blob(&20u64.to_le_bytes()).unwrap();
    let right = session.blob(&22u64.to_le_bytes()).unwrap();
    let add = session.lookup("add").unwrap();
    let metadata = session.blob(b"").unwrap();
    let combination = session.tree(&[metadata, add, left, right]).unwrap();
    let application = session.application(&combination).unwrap();
    assert_eq!(session.value_type(&application).unwrap(), ValueType::Thunk);
    let request = session.strict(&application).unwrap();
    let result = session.eval(&request).unwrap();
    let invalid = session.blob(b"not a combination").unwrap();
    assert!(session.call(&invalid).is_err());
    let trust_path = directory.join(".fix/trust");
    let trust = std::fs::read(&trust_path).unwrap();
    std::fs::write(&trust_path, b"invalid trust configuration").unwrap();
    assert!(session.eval(&request).is_err());
    std::fs::write(&trust_path, trust).unwrap();
    let repeated = session.eval(&request).unwrap();
    assert_eq!(
        session.save(&repeated).unwrap(),
        session.save(&result).unwrap()
    );
    let mut bytes = [0; 8];
    session.read(&result, 0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 42);
    session.label("answer", &result).unwrap();
    let name = session.save(&result).unwrap();
    session.close().unwrap();
    let mut session = client(&directory);
    let result = session.lookup("answer").unwrap();
    assert_eq!(session.save(&result).unwrap(), name);
    session.close().unwrap();
}

fn assert_trap(client: &mut Client, result: &fixcli::Value, code: u32) {
    assert!(!client.is_tag(result).unwrap());
    assert_eq!(client.len(result).unwrap(), 2);
    let marker = client.get(result, 0).unwrap();
    let reason = client.get(result, 1).unwrap();
    assert_eq!(client.len(&marker).unwrap(), 9);
    assert_eq!(client.len(&reason).unwrap(), 4);
    let mut bytes = [0; 9];
    client.read(&marker, 0, &mut bytes).unwrap();
    assert_eq!(&bytes, b"Wasm.trap");
    let mut bytes = [0; 4];
    client.read(&reason, 0, &mut bytes).unwrap();
    assert_eq!(u32::from_le_bytes(bytes), code);
}

#[test]
fn a_trapped_invocation_does_not_terminate_the_host() {
    let directory = repository("failed-invocation");
    let mut session = client(&directory);
    let add = session.lookup("add").unwrap();
    let metadata = session.blob(b"").unwrap();
    let combination = session.tree(&[metadata, add]).unwrap();
    let result = session.call(&combination).unwrap();
    assert_trap(&mut session, &result, 1);
    for number in [20u64, 21] {
        let metadata = session.blob(b"").unwrap();
        let add = session.lookup("add").unwrap();
        let argument = session.blob(&number.to_le_bytes()).unwrap();
        let combination = session
            .tree(&[metadata, add, argument.clone(), argument])
            .unwrap();
        let result = session.call(&combination).unwrap();
        let mut bytes = [0; 8];
        session.read(&result, 0, &mut bytes).unwrap();
        assert_eq!(u64::from_le_bytes(bytes), number * 2);
    }
    session.close().unwrap();
}

#[test]
fn coupon_persistence_modes() {
    fn claims(directory: &Path) -> std::collections::BTreeSet<String> {
        fixrepository::Repository::open(directory.join(".fix").to_str().unwrap())
            .unwrap()
            .unsigned()
            .claims(&[])
            .unwrap()
            .iter()
            .map(|tree| tree.as_handle().to_string())
            .collect()
    }
    for mode in [
        fixcli::Memoization::Auto,
        fixcli::Memoization::Always,
        fixcli::Memoization::Never,
    ] {
        let directory = repository(&format!("memoize-{mode}"));
        let before = claims(&directory);
        let mut session = Client::open(
            &directory,
            Options {
                trust: Some(directory.join(".fix/trust")),
                unsigned: true,
                memory_mib: 128,
                memoize: mode,
                ..Options::default()
            },
        )
        .unwrap();
        let left = session.blob(&20u64.to_le_bytes()).unwrap();
        let right = session.blob(&22u64.to_le_bytes()).unwrap();
        let add = session.lookup("add").unwrap();
        let metadata = session.blob(b"").unwrap();
        let combination = session.tree(&[metadata, add, left, right]).unwrap();
        let application = session.application(&combination).unwrap();
        let request = session.strict(&application).unwrap();
        let result = session.eval(&request).unwrap();
        let mut bytes = [0; 8];
        session.read(&result, 0, &mut bytes).unwrap();
        assert_eq!(u64::from_le_bytes(bytes), 42);
        let after = claims(&directory);
        if mode == fixcli::Memoization::Never {
            assert_eq!(after, before);
        } else {
            assert!(after.difference(&before).next().is_some());
        }
        let again = session.eval(&request).unwrap();
        session.read(&again, 0, &mut bytes).unwrap();
        assert_eq!(u64::from_le_bytes(bytes), 42);
        session.close().unwrap();
    }
}

#[test]
fn persistent_session_returns_guest_traps() {
    let directory = repository("failed-session");
    let mut session = client(&directory);
    session.reuse_vm();
    let add = session.lookup("add").unwrap();
    let metadata = session.blob(b"").unwrap();
    let combination = session.tree(&[metadata, add]).unwrap();
    let result = session.call(&combination).unwrap();
    assert_trap(&mut session, &result, 1);
    session.close().unwrap();
}

#[test]
fn perf_capture_preserves_results_and_exports_callchains() {
    let directory = repository("perf-capture");
    let mut session = client(&directory);
    let mut opaque = [0; 128];
    opaque[..6].copy_from_slice(b"\x7fELF\x02\x01");
    let opaque = session.blob(&opaque).unwrap();
    session.label("opaque", &opaque).unwrap();
    let left = session.blob(&20u64.to_le_bytes()).unwrap();
    let right = session.blob(&22u64.to_le_bytes()).unwrap();
    let add = session.lookup("add").unwrap();
    let metadata = session.blob(b"").unwrap();
    let combination = session.tree(&[metadata, add, left, right]).unwrap();
    let combination = session.save(&combination).unwrap();
    let expected = session.blob(&42u64.to_le_bytes()).unwrap();
    let expected = session.save(&expected).unwrap();
    session.close().unwrap();
    let profile = directory.join("evaluation.perf.data");
    let output = Command::new(input(env!("BACKEND")))
        .arg("--perf")
        .arg(&profile)
        .args(["--memory", "128", "-j", "2", "call", &combination])
        .current_dir(&directory)
        .env("FIX_UNSIGNED", "true")
        .env("FIX_TRUST", directory.join(".fix/trust"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
    let data = std::fs::read(profile).unwrap();
    assert_eq!(&data[..8], b"PERFILE2");
    let number = |offset| u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
    let attr_offset = number(24) as usize;
    let sample_type = number(attr_offset + 24);
    assert_ne!(sample_type & (1 << 5), 0);
    let start = number(40) as usize;
    let end = start + number(48) as usize;
    let mut cursor = start;
    while cursor < end {
        let size = u16::from_le_bytes(data[cursor + 6..cursor + 8].try_into().unwrap()) as usize;
        assert!(size >= 8 && cursor + size <= end);
        let kind = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
        if kind == 9 {
            let depth = number(cursor + 48) as usize;
            assert!(depth > 0 && 56 + depth * 8 <= size);
            assert_eq!(number(cursor + 8), number(cursor + 56));
        }
        cursor += size;
    }
    assert_eq!(cursor, end);
}
