use fixcertificates::Policy;
use fixhandle::{Blob, Handle, HandleView, Object, Tree};
use fixremote::Remote;
use fixremote_testing::Agent;
use fixrepository::Repository;
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, Command, Stdio},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("TEST_SRCDIR").unwrap())
        .join(std::env::var_os("TEST_WORKSPACE").unwrap())
        .join(name)
}

fn seed(directory: &Path, scheduler: &str) -> Repository {
    std::fs::create_dir_all(directory.join("toolchain")).unwrap();
    let output = Command::new(fixture(env!("SEED")))
        .args(["create", "."])
        .arg(fixture(env!("COMPILER")))
        .arg(fixture(env!("ADD")))
        .arg(fixture(env!("ADD")))
        .arg("toolchain")
        .arg(format!("scheduler={}", fixture(scheduler).display()))
        .arg(format!(
            "collector={}",
            fixture(env!("COLLECTOR")).display()
        ))
        .arg(format!("add={}", fixture(env!("ADD")).display()))
        .arg(format!("advisor={}", fixture(env!("ADVISOR")).display()))
        .arg(format!("fix-parser={}", fixture(env!("PARSER")).display()))
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Repository::open(directory.join(".fix").to_str().unwrap()).unwrap()
}

struct Worker {
    child: Child,
    logs: BufReader<ChildStderr>,
    startup: String,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
        let mut logs = std::mem::take(&mut self.startup);
        self.logs.read_to_string(&mut logs).unwrap();
        if !logs.is_empty() {
            eprintln!("worker: {logs}");
        }
    }
}

fn configuration(path: &Path, agent: &Agent, compiler: &str, peers: &[String]) {
    std::fs::create_dir_all(path).unwrap();
    fixcertificates::linux::write_config(
        &path.join("config"),
        &fixcertificates::linux::Config {
            identities: agent.public_keys.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    std::fs::write(
        path.join("authorized_keys"),
        agent
            .public_keys
            .iter()
            .chain(peers)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    std::fs::write(path.join("trusted_compilers"), compiler).unwrap();
    if let Ok(collector) =
        std::fs::read_to_string(path.parent().unwrap().join(".fix/labels/collector"))
    {
        std::fs::write(path.join("trusted_taggers"), collector).unwrap();
    }
}

fn sign_seed(directory: &Path, agent: &Agent, sign_on_import: bool) {
    let archive = directory.join("unsigned.tar");
    assert!(
        Command::new("tar")
            .args(["-cf"])
            .arg(&archive)
            .args([".fix"])
            .current_dir(directory)
            .status()
            .unwrap()
            .success()
    );
    let command = |arguments: &[&str]| {
        Command::new(fixture(env!("FIX")))
            .args(arguments)
            .current_dir(directory)
            .env("FIX_CONFIG_DIR", directory.join("user"))
            .env("SSH_AUTH_SOCK", &agent.socket)
            .env_remove("FIX_UNSIGNED")
            .env_remove("FIX_TRUST")
            .output()
            .unwrap()
    };
    let mut arguments = vec!["import", archive.to_str().unwrap()];
    if sign_on_import {
        arguments.push("--sign");
    }
    let imported = command(&arguments);
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    if !sign_on_import {
        assert!(!command(&["read-tree", "add"]).status.success());
        let signed = command(&["sign", "add"]);
        assert!(
            signed.status.success(),
            "{}",
            String::from_utf8_lossy(&signed.stderr)
        );
        assert!(command(&["read-tree", "add"]).status.success());
        assert!(!command(&["read-tree", "collector"]).status.success());
        let signed = command(&["sign", "--all"]);
        assert!(
            signed.status.success(),
            "{}",
            String::from_utf8_lossy(&signed.stderr)
        );
    }
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn run(
    source: &Path,
    agent: &Agent,
    request: Handle,
    remote: Option<&str>,
) -> std::process::Output {
    run_with_jobs(source, agent, request, remote, 2)
}

fn run_with_jobs(
    source: &Path,
    agent: &Agent,
    request: Handle,
    remote: Option<&str>,
    jobs: u64,
) -> std::process::Output {
    Command::new(fixture(env!("FIX")))
        .args([
            "--summarize",
            "--memory",
            "512",
            "-j",
            &jobs.to_string(),
            "schedule",
            "--trusted-tagger",
            "collector",
        ])
        .arg(request.to_string())
        .args(remote.into_iter().flat_map(|remote| ["--remote", remote]))
        .current_dir(source)
        .env("SSH_AUTH_SOCK", &agent.socket)
        .env("FIX_CONFIG_DIR", source.join("user"))
        .env_remove("FIX_TRUST")
        .env_remove("FIX_UNSIGNED")
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .output()
        .unwrap()
}

fn returned(repository: &Repository, output: std::process::Output) -> Tree {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let name = String::from_utf8(output.stdout).unwrap();
    let mut bytes = [0; 32];
    hex::decode_to_slice(name.trim(), &mut bytes).unwrap();
    Tree::try_from(repository.lookup(&bytes).unwrap()).unwrap()
}

fn start_worker(worker_dir: &Path, worker_agent: &Agent, jobs: u64) -> (Worker, String) {
    let mut child = Command::new(fixture(env!("FIX")))
        .args([
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--memory",
            "512",
            "-j",
            &jobs.to_string(),
        ])
        .current_dir(worker_dir)
        .env("SSH_AUTH_SOCK", &worker_agent.socket)
        .env("FIX_CONFIG_DIR", worker_dir.join("user"))
        .env_remove("FIX_TRUST")
        .env_remove("FIX_UNSIGNED")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut logs = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    let mut startup = String::new();
    let address = loop {
        line.clear();
        assert!(
            logs.read_line(&mut line).unwrap() > 0,
            "worker exited before listening: {startup}"
        );
        startup.push_str(&line);
        if let Some(address) = line.trim().strip_prefix("listening on ") {
            break address.to_string();
        }
    };
    (
        Worker {
            child,
            logs,
            startup,
        },
        address,
    )
}

#[test]
fn remote_evaluation_requires_trust_and_returns_a_coupon_for_the_original_input() {
    let directory = std::env::temp_dir().join(format!("fix-distributed-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let source_dir = directory.join("coordinator");
    let worker_dir = directory.join("worker");
    seed(&source_dir, env!("DISTRIBUTED"));
    seed(&worker_dir, env!("SERIAL"));
    let client_agent = Agent::with_keys(&directory.join("client-agent"), &[31, 32]);
    let worker_agent = Agent::with_keys(&directory.join("worker-agent"), &[37, 38]);
    let compiler = std::fs::read_to_string(source_dir.join(".fix/trust")).unwrap();
    configuration(&source_dir.join("user"), &client_agent, &compiler, &[]);
    configuration(
        &worker_dir.join("user"),
        &worker_agent,
        &compiler,
        &[client_agent.public_keys[1].clone()],
    );
    sign_seed(&source_dir, &client_agent, false);
    sign_seed(&worker_dir, &worker_agent, true);
    let source_policy = Policy {
        authorized_keys: client_agent
            .public_keys
            .iter()
            .chain(&worker_agent.public_keys)
            .cloned()
            .collect(),
        ..Policy::default()
    };
    let source = Repository::open(source_dir.join(".fix").to_str().unwrap())
        .unwrap()
        .with_policy(source_policy);
    let destination = Repository::open(worker_dir.join(".fix").to_str().unwrap())
        .unwrap()
        .with_policy(Policy {
            authorized_keys: worker_agent
                .public_keys
                .iter()
                .chain(&client_agent.public_keys)
                .cloned()
                .collect(),
            ..Policy::default()
        });
    let (_worker, address) = start_worker(&worker_dir, &worker_agent, 1);
    let _idle: Vec<_> = (0..4)
        .map(|_| {
            let mut connection =
                fixremote::linux::Connection(std::net::TcpStream::connect(&address).unwrap());
            connection
                .0
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            fixremote::protocol::send(
                &mut connection,
                fixremote::protocol::Message::RequestInfo { inventory: false },
            )
            .unwrap();
            assert!(matches!(
                fixremote::protocol::receive(&mut connection, 512 << 20).unwrap(),
                Some(fixremote::protocol::Message::Info(_))
            ));
            connection
        })
        .collect();

    source
        .set_remotes(vec![Remote {
            name: "worker".into(),
            address: format!("tcp://{address}"),
            scheduler: None,
            collector: None,
        }])
        .unwrap();

    let empty: Handle = source.create_blob(b"").unwrap().into();
    let add = source.read_label("add").unwrap();
    let first: Handle = source.create_blob(&20u64.to_le_bytes()).unwrap().into();
    let second: Handle = source.create_blob(&22u64.to_le_bytes()).unwrap().into();
    let combination = source.create_tree(&[empty, add, first, second]).unwrap();
    let input: Handle = combination.into_ref().application().strict().into();
    let scheduler = source.read_label("scheduler").unwrap();
    let collector = source.read_label("collector").unwrap();
    let mode: Handle = source.create_blob(b"eval").unwrap().into();
    let request: Handle = source
        .create_tree(&[empty, scheduler, mode, collector, input])
        .unwrap()
        .into();
    assert!(
        destination
            .lookup(combination.as_handle().as_bytes())
            .is_err()
    );

    let untrusted = run(&source_dir, &client_agent, request, None);
    assert!(!returned(&source, untrusted).is_tag());
    let prefix = [Blob::literal(&1u64.to_le_bytes()).unwrap().into(), input];
    assert!(source.claims(&prefix).unwrap().is_empty());
    assert!(!destination.claims(&prefix).unwrap().is_empty());
    configuration(
        &source_dir.join("user"),
        &client_agent,
        &compiler,
        &[worker_agent.public_keys[1].clone()],
    );
    source
        .set_remotes(vec![Remote {
            name: "worker".into(),
            address: "tcp://127.0.0.1:0".into(),
            scheduler: None,
            collector: None,
        }])
        .unwrap();
    let result = run(
        &source_dir,
        &client_agent,
        request,
        Some(&format!("worker=tcp://{address}")),
    );
    let coupon = returned(&source, result);
    assert!(
        coupon.is_tag(),
        "{:?}",
        source.read_tree(coupon).map(|fields| fields
            .iter()
            .map(|field| source
                .read_blob(Blob::try_from(*field).unwrap())
                .unwrap()
                .into_vec())
            .collect::<Vec<_>>())
    );
    assert_eq!(source.remotes().unwrap()[0].address, "tcp://127.0.0.1:0");
    let fields = source.read_tree(coupon).unwrap();
    assert!(fields[0].same_encoding(&collector));
    let claim = fixhandle::Ref::try_from(fields[1])
        .unwrap()
        .object_descriptor();
    let fields = source
        .read_tree(Tree::try_from(Handle::from(claim)).unwrap())
        .unwrap();
    assert!(fields[0].same_encoding(&Blob::literal(&1u64.to_le_bytes()).unwrap().into()));
    assert!(fields[1].same_encoding(&input));
    assert_eq!(
        source
            .read_blob(Blob::try_from(fields[2]).unwrap())
            .unwrap()
            .as_ref(),
        &42u64.to_le_bytes()
    );
    assert!(
        destination
            .lookup(combination.as_handle().as_bytes())
            .is_ok()
    );
    assert!(
        destination
            .has_backing(Object::try_from(Handle::from(combination)).unwrap())
            .unwrap()
    );
    assert!(destination.lookup(scheduler.as_bytes()).is_err());
    let ssh_dir = directory.join("stage-ssh-bin");
    std::fs::create_dir(&ssh_dir).unwrap();
    let ssh = ssh_dir.join("ssh");
    std::fs::write(
        &ssh,
        "#!/bin/sh\nprintf x >> \"$FIX_SSH_CONNECTIONS\"\nshift 3\nexport FIX_CONFIG_DIR=\"$FIX_SSH_CONFIG\" SSH_AUTH_SOCK=\"$FIX_SSH_AGENT\"\nexec sh -c \"$*\"\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(fixture(env!("FIX")), ssh_dir.join("fix")).unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ssh_result = Command::new(fixture(env!("FIX")))
        .args([
            "--memory",
            "512",
            "-j",
            "2",
            "schedule",
            "--trusted-tagger",
            "collector",
        ])
        .arg(request.to_string())
        .args([
            "--remote",
            &format!("worker=user@example:{}", worker_dir.display()),
        ])
        .current_dir(&source_dir)
        .env(
            "PATH",
            format!("{}:{}", ssh_dir.display(), std::env::var("PATH").unwrap()),
        )
        .env("FIX_CONFIG_DIR", source_dir.join("user"))
        .env("SSH_AUTH_SOCK", &client_agent.socket)
        .env("FIX_SSH_CONFIG", worker_dir.join("user"))
        .env("FIX_SSH_AGENT", &worker_agent.socket)
        .env("FIX_SSH_CONNECTIONS", directory.join("ssh-connections"))
        .env_remove("FIX_TRUST")
        .env_remove("FIX_UNSIGNED")
        .output()
        .unwrap();
    assert!(
        ssh_result.status.success(),
        "{}",
        String::from_utf8_lossy(&ssh_result.stderr)
    );
    assert!(returned(&source, ssh_result).is_tag());
    assert!(destination.lookup(scheduler.as_bytes()).is_ok());
    assert_eq!(
        std::fs::read(directory.join("ssh-connections")).unwrap(),
        b"x"
    );
    let certificates = source.tag_certificates(coupon.into()).unwrap();
    assert_eq!(certificates.len(), 1);
    let reloaded = |trust: bool| {
        configuration(
            &source_dir.join("user"),
            &client_agent,
            &compiler,
            if trust {
                core::slice::from_ref(&worker_agent.public_keys[1])
            } else {
                &[]
            },
        );
        Command::new(fixture(env!("FIX")))
            .args(["read-tree", &coupon.as_handle().to_string()])
            .current_dir(&source_dir)
            .env("FIX_CONFIG_DIR", source_dir.join("user"))
            .env_remove("FIX_UNSIGNED")
            .env_remove("SSH_AUTH_SOCK")
            .output()
            .unwrap()
    };
    assert!(!reloaded(false).status.success());
    assert!(reloaded(true).status.success());
    let archive = source_dir.join("signed.tar");
    let exported = Command::new(fixture(env!("FIX")))
        .args([
            "-j",
            "1",
            "--memory",
            "512",
            "--remote",
            &format!("worker=tcp://{address}"),
            "export",
            &format!("@handle(\"{}\")", coupon.as_handle()),
        ])
        .arg(&archive)
        .current_dir(&source_dir)
        .env("FIX_CONFIG_DIR", source_dir.join("user"))
        .env("SSH_AUTH_SOCK", &client_agent.socket)
        .env_remove("FIX_UNSIGNED")
        .env_remove("FIX_TRUST")
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let mut archived_certificates = Vec::new();
    for key in [
        &worker_agent.public_keys[1],
        &client_agent.public_keys[0],
        &client_agent.public_keys[1],
    ] {
        let signer = Blob::canonical(key.as_bytes()).unwrap();
        let archived = Command::new("tar")
            .arg("-xOf")
            .arg(&archive)
            .arg(format!(
                ".fix/tags/signatures/{}/{}",
                coupon.as_handle(),
                signer.as_handle()
            ))
            .output()
            .unwrap();
        assert!(archived.status.success());
        let mut data_key: [u8; 32] = archived.stdout.try_into().unwrap();
        data_key[30..].fill(0xff);
        let archived_data = Command::new("tar")
            .arg("-xOf")
            .arg(&archive)
            .arg(format!(".fix/data/{}", hex::encode(data_key)))
            .output()
            .unwrap();
        assert!(archived_data.status.success());
        let certificate = fixcertificates::Certificate::decode(&archived_data.stdout).unwrap();
        assert_eq!(&certificate.signer().unwrap(), key);
        archived_certificates.push(certificate);
    }
    assert!(archived_certificates.iter().any(|certificate| {
        certificate
            .verify(
                coupon.as_handle().as_bytes(),
                &Policy {
                    authorized_keys: vec![client_agent.public_keys[1].clone()],
                    ..Policy::default()
                },
            )
            .is_ok()
    }));
    let original = source.tag_certificates(coupon.into()).unwrap();
    assert_eq!(original.len(), 1);
    original[0]
        .verify(
            coupon.as_handle().as_bytes(),
            &Policy {
                authorized_keys: vec![worker_agent.public_keys[1].clone()],
                ..Policy::default()
            },
        )
        .unwrap();
    let ssh_dir = directory.join("ssh-bin");
    std::fs::create_dir(&ssh_dir).unwrap();
    let ssh = ssh_dir.join("ssh");
    std::fs::write(&ssh, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FIX_SSH_ARGS\"\nexport FIX_CONFIG_DIR=\"$FIX_SSH_CONFIG\" SSH_AUTH_SOCK=\"$FIX_SSH_AUTH\"\nexec \"$FIX_SSH_TEST_FIX\" -C \"$FIX_SSH_TEST_REPOSITORY\" --memory 512 -j 1 serve --stdio\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let next = source
        .create_tree(&[
            empty,
            add,
            source.create_blob(&23u64.to_le_bytes()).unwrap().into(),
            source.create_blob(&19u64.to_le_bytes()).unwrap().into(),
        ])
        .unwrap();
    let input: Handle = next.into_ref().application().strict().into();
    let request: Handle = source
        .create_tree(&[empty, scheduler, mode, collector, input])
        .unwrap()
        .into();
    let remote_scheduler = destination.read_label("scheduler").unwrap().to_string();
    let remote_collector = destination.read_label("collector").unwrap().to_string();
    let scheduler_override = format!("worker={remote_scheduler}");
    let collector_override = format!("worker={remote_collector}");
    let ssh_args = directory.join("ssh-args");
    let output = Command::new(fixture(env!("FIX")))
        .args([
            "--memory",
            "512",
            "-j",
            "2",
            "--remote-scheduler",
            &scheduler_override,
            "--remote-collector",
            &collector_override,
            "schedule",
            "--trusted-tagger",
            "collector",
        ])
        .arg(request.to_string())
        .args(["--remote", "worker=user@example:/repository"])
        .current_dir(&source_dir)
        .env(
            "PATH",
            format!("{}:{}", ssh_dir.display(), std::env::var("PATH").unwrap()),
        )
        .env("FIX_SSH_TEST_FIX", fixture(env!("FIX")))
        .env("FIX_SSH_TEST_REPOSITORY", &worker_dir)
        .env("SSH_AUTH_SOCK", &client_agent.socket)
        .env("FIX_SSH_CONFIG", worker_dir.join("user"))
        .env("FIX_SSH_AUTH", &worker_agent.socket)
        .env("FIX_SSH_ARGS", &ssh_args)
        .env("FIX_CONFIG_DIR", source_dir.join("user"))
        .env_remove("FIX_TRUST")
        .env_remove("FIX_UNSIGNED")
        .output()
        .unwrap();
    let coupon = returned(&source, output);
    let arguments = std::fs::read_to_string(&ssh_args).unwrap();
    assert!(arguments.contains(&format!("--scheduler\n'{remote_scheduler}'\n")));
    assert!(arguments.contains(&format!("--collector\n'{remote_collector}'\n")));
    let prefix = [Blob::literal(&1u64.to_le_bytes()).unwrap().into(), input];
    assert!(!destination.claims(&prefix).unwrap().is_empty());
    let coupon_fields = source.read_tree(coupon).unwrap();
    let claim = fixhandle::Ref::try_from(coupon_fields[1])
        .unwrap()
        .object_descriptor();
    let fields = source
        .read_tree(Tree::try_from(Handle::from(claim)).unwrap())
        .unwrap();
    assert!(fields[1].same_encoding(&input));
    assert_eq!(
        source
            .read_blob(Blob::try_from(fields[2]).unwrap())
            .unwrap()
            .as_ref(),
        &42u64.to_le_bytes()
    );

    std::fs::remove_file(&ssh_args).unwrap();
    let evaluation = Command::new(fixture(env!("FIX")))
        .args([
            "--memory",
            "512",
            "-j",
            "2",
            "--remote",
            "worker=user@example:/repository",
            "--remote-scheduler",
            &scheduler_override,
            "--remote-collector",
            &collector_override,
            "eval",
            "--encode",
            "#(\"\" @label(\"add\") 25u64 17u64)",
        ])
        .current_dir(&source_dir)
        .env(
            "PATH",
            format!("{}:{}", ssh_dir.display(), std::env::var("PATH").unwrap()),
        )
        .env("FIX_SSH_TEST_FIX", fixture(env!("FIX")))
        .env("FIX_SSH_TEST_REPOSITORY", &worker_dir)
        .env("FIX_SSH_CONFIG", worker_dir.join("user"))
        .env("FIX_SSH_AUTH", &worker_agent.socket)
        .env("FIX_SSH_ARGS", &ssh_args)
        .env("SSH_AUTH_SOCK", &client_agent.socket)
        .env("FIX_CONFIG_DIR", source_dir.join("user"))
        .env_remove("FIX_TRUST")
        .env_remove("FIX_UNSIGNED")
        .output()
        .unwrap();
    assert!(
        evaluation.status.success(),
        "{}",
        String::from_utf8_lossy(&evaluation.stderr)
    );
    let arguments = std::fs::read_to_string(&ssh_args).unwrap();
    assert!(arguments.contains(&format!("--scheduler\n'{remote_scheduler}'\n")));
    assert!(arguments.contains(&format!("--collector\n'{remote_collector}'\n")));
}

#[test]
fn remote_commands_are_repository_local_and_trust_commands_are_per_user() {
    let directory = std::env::temp_dir().join(format!("fix-configuration-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let home = directory.join("home");
    std::fs::create_dir(&home).unwrap();
    Repository::init(home.join(".fix").to_str().unwrap()).unwrap();
    let first = directory.join("first");
    let second = directory.join("second");
    Repository::init(first.join(".fix").to_str().unwrap()).unwrap();
    let other = Repository::init(second.join(".fix").to_str().unwrap()).unwrap();
    let command = |cwd: &Path, arguments: &[&str], success: bool| {
        let output = Command::new(fixture(env!("FIX")))
            .args(arguments)
            .current_dir(cwd)
            .env("HOME", &home)
            .env_remove("FIX_CONFIG_DIR")
            .env_remove("FIX_UNSIGNED")
            .env_remove("FIX_TRUST")
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    command(
        &first,
        &["remote", "add", "origin", "tcp://first:1000"],
        true,
    );
    command(
        &first,
        &["remote", "add", "origin", "tcp://second:1000"],
        false,
    );
    assert_eq!(
        command(&first, &["remote", "get-url", "origin"], true),
        "tcp://first:1000\n"
    );
    assert!(other.remotes().unwrap().is_empty());
    command(&home, &["config", "--global", "scheduler", "serial"], true);
    assert_eq!(command(&first, &["config", "scheduler"], true), "serial\n");
    command(&first, &["config", "scheduler", "parallel"], true);
    assert_eq!(
        command(&first, &["config", "scheduler"], true),
        "parallel\n"
    );
    assert_eq!(command(&second, &["config", "scheduler"], true), "serial\n");
    command(&first, &["config", "--unset", "scheduler"], true);
    assert_eq!(command(&first, &["config", "scheduler"], true), "serial\n");
    command(&first, &["remote", "rename", "origin", "worker"], true);
    command(
        &first,
        &["remote", "set-url", "worker", "tcp://second:1000"],
        true,
    );
    assert_eq!(
        command(&first, &["remote", "get-url", "worker"], true),
        "tcp://second:1000\n"
    );
    command(
        &first,
        &["remote", "set-scheduler", "worker", "remote-scheduler"],
        true,
    );
    command(
        &first,
        &["remote", "set-collector", "worker", "remote-collector"],
        true,
    );
    assert_eq!(
        command(&first, &["remote", "get-scheduler", "worker"], true),
        "remote-scheduler\n"
    );
    assert_eq!(
        command(&first, &["remote", "get-collector", "worker"], true),
        "remote-collector\n"
    );
    command(&first, &["remote", "remove", "worker"], true);
    assert!(command(&first, &["remote", "list"], true).is_empty());
    let agent = Agent::new(&directory.join("agent"), 41);
    let key = directory.join("identity.pub");
    std::fs::write(&key, &agent.public_key).unwrap();
    let key = key.to_str().unwrap();
    command(&home, &["trust", "add-key", key], true);
    let tagger = Tree::empty().as_handle().to_string();
    command(&home, &["trust", "add-tagger", &tagger], true);
    assert!(command(&second, &["trust", "list"], true).contains(&format!("tagger {tagger}")));
    command(&first, &["trust", "remove-tagger", &tagger], true);
    assert!(!command(&second, &["trust", "list"], true).contains("tagger "));

    command(&home, &["trust", "add-identity", key], true);
    assert_eq!(command(&second, &["config", "scheduler"], true), "serial\n");
    let compiler = Blob::canonical(&[3; 64])
        .unwrap()
        .into_ref()
        .as_handle()
        .to_string();
    let compiler_name = *Blob::canonical(&[3; 64])
        .unwrap()
        .into_ref()
        .as_handle()
        .as_bytes();
    for repository in [
        Repository::open(first.join(".fix").to_str().unwrap()).unwrap(),
        other,
    ] {
        repository
            .set_remote_refs("origin", vec![("compiler".into(), compiler_name)])
            .unwrap();
    }
    command(&first, &["trust", "add-compiler", "origin:compiler"], true);
    assert_eq!(
        std::fs::read_to_string(home.join(".config/fix/authorized_keys"))
            .unwrap()
            .trim(),
        agent.public_key
    );
    let listed = command(&home, &["trust", "list"], true);
    assert!(listed.contains(&compiler));
    assert!(listed.contains(&agent.public_key));
    command(&second, &["trust", "remove-key", key], true);
    command(&second, &["trust", "remove-identity", key], true);
    command(
        &second,
        &["trust", "remove-compiler", "origin:compiler"],
        true,
    );
    assert!(command(&home, &["trust", "list"], true).is_empty());

    let agent = Agent::with_keys(&directory.join("active-agent"), &[43, 44]);
    for _ in 0..2 {
        let output = Command::new(fixture(env!("FIX")))
            .args(["trust", "add-agent"])
            .env("HOME", &home)
            .env("SSH_AUTH_SOCK", &agent.socket)
            .env_remove("FIX_CONFIG_DIR")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let listed = command(&home, &["trust", "list"], true);
    for key in &agent.public_keys {
        assert_eq!(listed.matches(&format!("key {key}")).count(), 1);
        assert_eq!(listed.matches(&format!("identity {key}")).count(), 1);
    }
}

#[test]
fn init_preserves_compiled_programs_from_an_archive() {
    let directory = std::env::temp_dir().join(format!("fix-init-archive-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let source = directory.join("source");
    seed(&source, env!("SERIAL"));
    let parser = std::fs::read_to_string(source.join(".fix/labels/fix-parser")).unwrap();
    let archive = directory.join("seed.tar");
    assert!(
        Command::new("tar")
            .arg("-cf")
            .arg(&archive)
            .arg(".fix")
            .current_dir(&source)
            .status()
            .unwrap()
            .success()
    );

    let target = directory.join("target");
    std::fs::create_dir(&target).unwrap();
    let agent = Agent::new(&directory.join("agent"), 46);
    let compiler = std::fs::read_to_string(source.join(".fix/trust")).unwrap();
    configuration(&target.join("user"), &agent, &compiler, &[]);
    let absent_tagger = Tree::canonical(&[Blob::literal(b"other repository").unwrap().into()])
        .unwrap()
        .tag_descriptor();
    std::fs::write(
        target.join("user/trusted_taggers"),
        absent_tagger.as_handle().to_string(),
    )
    .unwrap();
    let run = |arguments: &[&str]| {
        let output = Command::new(fixture(env!("FIX")))
            .args(arguments)
            .current_dir(&target)
            .env("FIX_CONFIG_DIR", target.join("user"))
            .env("SSH_AUTH_SOCK", &agent.socket)
            .env_remove("FIX_UNSIGNED")
            .env_remove("FIX_TRUST")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&["init"]);
    run(&["import", archive.to_str().unwrap(), "--sign"]);
    assert_eq!(
        std::fs::read_to_string(target.join(".fix/labels/fix-parser")).unwrap(),
        parser
    );
    let untrusted = Command::new(fixture(env!("FIX")))
        .args(["--memory", "512", "-j", "1", "eval", "1u64"])
        .current_dir(&target)
        .env("FIX_CONFIG_DIR", target.join("user"))
        .env("SSH_AUTH_SOCK", &agent.socket)
        .env_remove("FIX_UNSIGNED")
        .env_remove("FIX_TRUST")
        .output()
        .unwrap();
    assert!(!untrusted.status.success());
    assert!(String::from_utf8_lossy(&untrusted.stderr).contains("not a trusted tagger"));
    std::fs::write(
        target.join("user/trusted_taggers"),
        std::fs::read_to_string(target.join(".fix/labels/collector")).unwrap(),
    )
    .unwrap();
    run(&["--memory", "512", "-j", "1", "eval", "1u64"]);
}

#[test]
fn remote_labels_and_transfer_modes() {
    let directory = std::env::temp_dir().join(format!("fix-remote-labels-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let server = directory.join("server");
    let repository = seed(&server, env!("SERIAL"));
    let mut programs = Command::new(fixture(env!("SEED")));
    programs.args(["add", server.to_str().unwrap()]);
    for (name, executable) in [
        ("parallel", env!("PARALLEL")),
        ("distributed", env!("DISTRIBUTED")),
        ("onepass", env!("ONEPASS")),
        ("hint", env!("HINT")),
    ] {
        programs.arg(format!("{name}={}", fixture(executable).display()));
    }
    let programs = programs.output().unwrap();
    assert!(
        programs.status.success(),
        "{}",
        String::from_utf8_lossy(&programs.stderr)
    );
    let payload = vec![11; (2 << 20) + 37];
    let original = repository.create_blob(&payload).unwrap();
    repository.write_label("payload", original.into()).unwrap();
    let agent = Agent::new(&directory.join("agent"), 45);
    let compiler = std::fs::read_to_string(server.join(".fix/trust")).unwrap();
    configuration(&server.join("user"), &agent, &compiler, &[]);
    sign_seed(&server, &agent, true);

    let data = directory.join("data");
    let bootstrap = data.join("fix/bootstrap");
    seed(&bootstrap, env!("SERIAL"));
    let added = Command::new(fixture(env!("SEED")))
        .args(["add", bootstrap.to_str().unwrap()])
        .arg(format!("hint={}", fixture(env!("HINT")).display()))
        .output()
        .unwrap();
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    configuration(&bootstrap.join("user"), &agent, &compiler, &[]);
    sign_seed(&bootstrap, &agent, true);
    let ssh_dir = directory.join("bin");
    std::fs::create_dir(&ssh_dir).unwrap();
    let ssh = ssh_dir.join("ssh");
    std::fs::write(
        &ssh,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FIX_SSH_ARGS\"\nexec \"$FIX_SSH_TEST_FIX\" -C \"$FIX_SSH_TEST_REPOSITORY\" --unsigned --memory 512 -j 1 serve --stdio\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let ssh_args = directory.join("ssh-args");
    let command = |cwd: &Path, args: &[&str]| {
        Command::new(fixture(env!("FIX")))
            .args(args)
            .current_dir(cwd)
            .env(
                "PATH",
                format!("{}:{}", ssh_dir.display(), std::env::var("PATH").unwrap()),
            )
            .env("FIX_SSH_TEST_FIX", fixture(env!("FIX")))
            .env("FIX_SSH_TEST_REPOSITORY", &server)
            .env("FIX_SSH_ARGS", &ssh_args)
            .env_remove("FIX_UNSIGNED")
            .env("FIX_TRUST", server.join(".fix/trust"))
            .env("FIX_CONFIG_DIR", server.join("user"))
            .env("XDG_DATA_HOME", &data)
            .env("SSH_AUTH_SOCK", &agent.socket)
            .env_remove("BUILD_WORKSPACE_DIRECTORY")
            .output()
            .unwrap()
    };
    let run = |cwd: &Path, args: &[&str], success: bool| {
        let output = command(cwd, args);
        assert_eq!(
            output.status.success(),
            success,
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    for (setting, label) in [
        ("scheduler", "scheduler"),
        ("collector", "collector"),
        ("parser", "fix-parser"),
    ] {
        run(
            &server,
            &[
                "config",
                "--global",
                setting,
                &Repository::open(repository.path())
                    .unwrap()
                    .unsigned()
                    .read_label(label)
                    .unwrap()
                    .to_string(),
            ],
            true,
        );
    }
    let fields = [
        Blob::literal(b"").unwrap().into(),
        Repository::open(repository.path())
            .unwrap()
            .unsigned()
            .read_label("add")
            .unwrap(),
        Blob::literal(&1u64.to_le_bytes()).unwrap().into(),
        Blob::literal(&2u64.to_le_bytes()).unwrap().into(),
    ];
    let combination = Repository::open(repository.path())
        .unwrap()
        .unsigned()
        .create_tree(&fields)
        .unwrap();
    let thunk: fixhandle::Handle = combination.into_ref().application().into();
    repository.write_label("sum", thunk).unwrap();
    let address = "user@example:/remote";
    run(&directory, &["clone", address, "client"], true);
    let client_dir = directory.join("client");
    let local = Repository::open(client_dir.join(".fix").to_str().unwrap()).unwrap();
    assert!(
        local
            .read_label("origin:payload")
            .unwrap()
            .same_encoding(&original.into_ref().into())
    );
    assert!(
        local
            .read_label("origin:sum")
            .unwrap()
            .same_encoding(&thunk)
    );
    assert!(!local.has_backing(original.into()).unwrap());
    std::fs::remove_file(&ssh_args).unwrap();
    assert_eq!(
        run(&client_dir, &["@label(\"origin:sum\")"], true).trim(),
        thunk.to_string()
    );
    assert!(!ssh_args.exists());
    for scheduler in ["parallel", "distributed", "onepass", "hint"] {
        run(&server, &["config", "scheduler", scheduler], true);
        run(
            &directory,
            &["clone", address, &format!("clone-{scheduler}")],
            true,
        );
    }
    run(&server, &["config", "scheduler", "scheduler"], true);
    run(
        &client_dir,
        &["pull", "--shallow", "origin", "payload", "payload-ref"],
        true,
    );
    assert!(matches!(
        local.read_label("payload-ref").unwrap().view(),
        HandleView::Ref(_)
    ));
    assert!(!local.has_backing(original.into()).unwrap());
    run(&directory, &["clone", address, "hint-client"], true);
    let hint_client = directory.join("hint-client");
    let hint = Repository::open(repository.path())
        .unwrap()
        .unsigned()
        .read_label("hint")
        .unwrap()
        .to_string();
    run(&hint_client, &["config", "scheduler", &hint], true);
    let pulled = command(&hint_client, &["pull"]);
    assert!(
        pulled.status.success(),
        "{}",
        String::from_utf8_lossy(&pulled.stderr)
    );
    let hint_store = Repository::open(hint_client.join(".fix").to_str().unwrap()).unwrap();
    assert!(
        !hint_store.has_backing(original.into()).unwrap(),
        "{}",
        String::from_utf8_lossy(&pulled.stderr)
    );
    assert!(
        hint_store
            .read_label("origin:payload")
            .unwrap()
            .same_encoding(&original.into_ref().into())
    );
    let selection = local
        .create_tree(&[
            original.into_ref().into(),
            Blob::literal(&1u64.to_le_bytes()).unwrap().into(),
            Blob::literal(&4u64.to_le_bytes()).unwrap().into(),
        ])
        .unwrap()
        .into_ref()
        .selection();
    local.write_label("slice", selection.into()).unwrap();
    let selection_output = command(&client_dir, &["eval", "--encode", "@label(\"slice\")"]);
    assert!(
        selection_output.status.success(),
        "{}",
        String::from_utf8_lossy(&selection_output.stderr)
    );
    let selection_progress = String::from_utf8_lossy(&selection_output.stderr);
    assert!(
        selection_progress
            .lines()
            .any(|line| line.contains("local: Load ")
                && line.contains("(from origin)")
                && (line.contains("origin:payload") || line.contains("payload-ref"))),
        "{selection_progress}"
    );
    let selected = String::from_utf8(selection_output.stdout).unwrap();
    assert_eq!(
        selected.trim(),
        Blob::literal(&payload[1..4])
            .unwrap()
            .as_handle()
            .to_string()
    );
    let file_address = format!("file://{}", repository.path());
    run(&directory, &["clone", &file_address, "file-client"], true);
    let file_client = directory.join("file-client");
    let loaded = command(
        &file_client,
        &[
            "--summarize",
            "pull",
            "--strict",
            "origin",
            "payload",
            "payload",
        ],
    );
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    let diagnostics = String::from_utf8_lossy(&loaded.stderr);
    assert!(
        diagnostics.contains("local: Load *'origin:payload ("),
        "{diagnostics}"
    );
    let summary = diagnostics.split("\nNode ").nth(1).expect("node summary");
    let nodes: Vec<_> = summary
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    assert_eq!(nodes, ["local"]);
    assert_eq!(
        Repository::open(file_client.join(".fix").to_str().unwrap())
            .unwrap()
            .read_blob(original)
            .unwrap()
            .as_ref(),
        payload
    );
    run(
        &client_dir,
        &["pull", "--strict", "origin", "payload", "payload"],
        true,
    );
    assert!(
        local
            .read_label("payload")
            .unwrap()
            .same_encoding(&original.into())
    );
    assert_eq!(local.read_blob(original).unwrap().as_ref(), payload);
    local
        .write_label("resident", original.into_ref().into())
        .unwrap();
    let resident_selection = local
        .create_tree(&[
            original.into_ref().into(),
            Blob::literal(&2u64.to_le_bytes()).unwrap().into(),
            Blob::literal(&5u64.to_le_bytes()).unwrap().into(),
        ])
        .unwrap()
        .into_ref()
        .selection();
    local
        .write_label("resident-slice", resident_selection.into())
        .unwrap();
    let resident = command(
        &client_dir,
        &["eval", "--encode", "@label(\"resident-slice\")"],
    );
    assert!(
        resident.status.success(),
        "{}",
        String::from_utf8_lossy(&resident.stderr)
    );
    let diagnostics = String::from_utf8_lossy(&resident.stderr);
    assert!(
        diagnostics.lines().any(|line| line.contains("local: Load ")
            && (line.contains("payload") || line.contains("resident"))
            && !line.contains("(from ")),
        "{diagnostics}"
    );

    run(
        &client_dir,
        &["pull", "--exact", "origin", "sum", "sum"],
        true,
    );
    assert!(local.read_label("sum").unwrap().same_encoding(&thunk));
    assert!(local.has_backing(combination.into()).unwrap());
    run(
        &client_dir,
        &["push", "--shallow", "origin", "payload", "copy-ref"],
        true,
    );
    assert!(
        repository
            .read_label("copy-ref")
            .unwrap()
            .same_encoding(&original.into_ref().into())
    );
    run(
        &client_dir,
        &["push", "--strict", "origin", "payload", "copy-object"],
        true,
    );
    assert!(
        repository
            .read_label("copy-object")
            .unwrap()
            .same_encoding(&original.into())
    );
    run(
        &client_dir,
        &["push", "--exact", "origin", "sum", "copy-thunk"],
        true,
    );
    assert!(
        repository
            .read_label("copy-thunk")
            .unwrap()
            .same_encoding(&thunk)
    );
    run(
        &client_dir,
        &["config", "--global", "pull.mode", "exact"],
        true,
    );
    run(&client_dir, &["config", "pull.mode", "strict"], true);
    run(
        &client_dir,
        &["pull", "origin", "payload", "configured"],
        true,
    );
    assert!(matches!(
        local.read_label("configured").unwrap().view(),
        HandleView::Object(_)
    ));
    run(
        &client_dir,
        &["pull", "--shallow", "origin", "payload", "configured"],
        true,
    );
    assert!(matches!(
        local.read_label("configured").unwrap().view(),
        HandleView::Ref(_)
    ));
    run(
        &client_dir,
        &["config", "--global", "push.mode", "exact"],
        true,
    );
    run(&client_dir, &["config", "push.mode", "strict"], true);
    run(
        &client_dir,
        &["push", "origin", "payload", "configured"],
        true,
    );
    assert!(matches!(
        repository.read_label("configured").unwrap().view(),
        HandleView::Object(_)
    ));
    run(
        &client_dir,
        &["push", "--shallow", "origin", "payload", "configured"],
        true,
    );
    assert!(matches!(
        repository.read_label("configured").unwrap().view(),
        HandleView::Ref(_)
    ));
    assert!(run(&client_dir, &["remote", "labels", "origin"], true).contains("origin:sum"));
    let args = ["--summarize", "pull", "--shallow", "origin"];
    let first = command(&client_dir, &args);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_report = String::from_utf8_lossy(&first.stderr);
    assert_eq!(
        first_report
            .lines()
            .filter(|line| line.starts_with("Pull ") && line.contains(" completed ("))
            .count(),
        1,
        "{first_report}"
    );
    let repeated = command(&client_dir, &args);
    assert!(
        repeated.status.success(),
        "{}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    let report = String::from_utf8_lossy(&repeated.stderr);
    let local = report
        .lines()
        .find(|line| line.starts_with("local "))
        .unwrap();
    let columns: Vec<_> = local.split_whitespace().collect();
    assert_eq!(
        &columns[1..3],
        &["0", "0"],
        "unchanged pull performed computation: {report}"
    );
}

#[test]
fn signing_adds_identities_and_import_sign_is_scoped_to_the_archive() {
    let directory = std::env::temp_dir().join(format!("fix-signing-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let source_dir = directory.join("source");
    let target_dir = directory.join("target");
    let source = Repository::init(source_dir.join(".fix").to_str().unwrap())
        .unwrap()
        .unsigned();
    let target = Repository::init(target_dir.join(".fix").to_str().unwrap())
        .unwrap()
        .unsigned();
    let leaf = |repository: &Repository, label: &str| {
        let fields = [
            Tree::empty().into(),
            Blob::literal(label.as_bytes()).unwrap().into(),
        ];
        let tag = Tree::canonical(&fields).unwrap().tag_descriptor();
        repository
            .publish(
                tag.into(),
                &fields
                    .iter()
                    .flat_map(|field| *field.as_bytes())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        repository.write_label(label, tag.into()).unwrap();
        tag
    };
    let first = leaf(&source, "first");
    leaf(&target, "pending");
    let author = Agent::new(&directory.join("author-agent"), 43);
    let user = Agent::with_keys(&directory.join("user-agent"), &[47, 48]);
    configuration(&source_dir.join("user"), &author, "", &[]);
    configuration(&target_dir.join("user"), &user, "", &[]);
    let command = |repository: &Path, agent: Option<&Agent>, arguments: &[&str], success: bool| {
        let mut command = Command::new(fixture(env!("FIX")));
        command
            .args(arguments)
            .current_dir(repository)
            .env("FIX_CONFIG_DIR", repository.join("user"))
            .env_remove("FIX_UNSIGNED")
            .env_remove("FIX_TRUST")
            .env_remove("SSH_AUTH_SOCK");
        if let Some(agent) = agent {
            command.env("SSH_AUTH_SOCK", &agent.socket);
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    command(&source_dir, Some(&author), &["sign", "first"], true);
    let archive = directory.join("signed.tar");
    assert!(
        Command::new("tar")
            .arg("-cf")
            .arg(&archive)
            .arg(".fix")
            .current_dir(&source_dir)
            .status()
            .unwrap()
            .success()
    );
    command(
        &target_dir,
        None,
        &["import", archive.to_str().unwrap()],
        true,
    );
    command(&target_dir, None, &["read-tree", "first"], false);
    command(
        &target_dir,
        Some(&user),
        &["import", "--sign", archive.to_str().unwrap()],
        true,
    );
    command(&target_dir, None, &["read-tree", "first"], true);
    command(&target_dir, None, &["read-tree", "pending"], false);
    let certificate = || {
        let repository = Repository::open(target.path())
            .unwrap()
            .with_policy(Policy {
                authorized_keys: user.public_keys.clone(),
                ..Policy::default()
            });
        repository.tag_certificates(first.into()).unwrap()
    };
    let signed = certificate();
    assert_eq!(signed.len(), 3);
    for key in [&author.public_key, &user.public_keys[1]] {
        assert!(signed.iter().any(|certificate| {
            certificate
                .verify(
                    first.as_handle().as_bytes(),
                    &Policy {
                        authorized_keys: vec![key.clone()],
                        ..Policy::default()
                    },
                )
                .is_ok()
        }));
    }
    command(
        &target_dir,
        Some(&user),
        &["sign", &first.as_handle().to_string()],
        true,
    );
    assert_eq!(certificate().len(), 3);
    command(&target_dir, Some(&user), &["sign", "--all"], true);
    command(&target_dir, None, &["read-tree", "pending"], true);
    let bad = leaf(&target, "bad");
    std::fs::write(
        target_dir
            .join(".fix/objects")
            .join(bad.as_handle().to_string()),
        [0; 64],
    )
    .unwrap();
    command(&target_dir, Some(&user), &["sign", "bad"], false);
}

#[test]
fn placement_policies_use_inventory_memory_and_advice_with_conservative_fallbacks() {
    let directory = std::env::temp_dir().join(format!("fix-placement-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let source_agent = Agent::new(&directory.join("source-agent"), 81);
    let worker_agent = Agent::new(&directory.join("placement-worker-agent"), 82);
    let worker_dir = directory.join("worker");
    seed(&worker_dir, env!("PARALLEL"));
    let compiler = std::fs::read_to_string(worker_dir.join(".fix/trust")).unwrap();
    configuration(
        &worker_dir.join("user"),
        &worker_agent,
        &compiler,
        std::slice::from_ref(&source_agent.public_key),
    );
    sign_seed(&worker_dir, &worker_agent, true);
    let policy = Policy {
        authorized_keys: vec![
            source_agent.public_key.clone(),
            worker_agent.public_key.clone(),
        ],
        ..Policy::default()
    };
    let destination = Repository::open(worker_dir.join(".fix").to_str().unwrap())
        .unwrap()
        .with_policy(policy.clone());
    let (_worker, address) = start_worker(&worker_dir, &worker_agent, 2);
    for (index, scheduler) in [(0u64, env!("ONEPASS")), (1u64, env!("HINT"))] {
        let source_dir = directory.join(format!("source-{index}"));
        seed(&source_dir, scheduler);
        configuration(
            &source_dir.join("user"),
            &source_agent,
            &compiler,
            std::slice::from_ref(&worker_agent.public_key),
        );
        sign_seed(&source_dir, &source_agent, true);
        let source = Repository::open(source_dir.join(".fix").to_str().unwrap())
            .unwrap()
            .with_policy(policy.clone());
        source
            .set_remotes(vec![
                Remote {
                    name: "storage".into(),
                    address: format!("file://{}", worker_dir.join(".fix").display()),
                    scheduler: None,
                    collector: None,
                },
                Remote {
                    name: "worker".into(),
                    address: format!("tcp://{address}"),
                    scheduler: None,
                    collector: None,
                },
            ])
            .unwrap();
        let empty: Handle = Blob::literal(b"").unwrap().into();
        let mode: Handle = Blob::literal(b"eval").unwrap().into();
        let collector = source.read_label("collector").unwrap();
        let function = source.read_label("add").unwrap();
        let advisor = source.read_label("advisor").unwrap();
        for case in 0u64..7 {
            let first: Handle = Blob::literal(&(20 + index * 10 + case).to_le_bytes())
                .unwrap()
                .into();
            let second: Handle = Blob::literal(&22u64.to_le_bytes()).unwrap().into();
            let large = source.create_blob(&(1u64 << 30).to_le_bytes()).unwrap();
            let limit: Handle = if case == 1 { large.into() } else { empty };
            let remote_data = if case == 5 {
                Some(source.create_blob(&vec![index as u8; 4096]).unwrap())
            } else {
                None
            };
            let mut arguments = vec![limit, function, first, second];
            if let Some(data) = remote_data {
                arguments.push(data.into_ref().into());
            }
            let call = source.create_tree(&arguments).unwrap();
            let thunk = call.into_ref().application();
            let input: Handle = if !(2..5).contains(&case) {
                thunk.strict().into()
            } else {
                let output_size = (64u64 << 20).to_le_bytes();
                let output = source
                    .create_blob(if case == 2 { &output_size } else { b"bad" })
                    .unwrap();
                let parallel: Handle = Blob::literal(&u64::from(case == 4).to_le_bytes())
                    .unwrap()
                    .into();
                let advised = source
                    .create_tree(&[large.into(), advisor, call.into(), output.into(), parallel])
                    .unwrap();
                advised.into_ref().application().strict().into()
            };
            let (mut transfer, backing) = fixremote::propose_export_full(&source, input).unwrap();
            for record in &mut transfer.records {
                if backing.contains(&record.name) {
                    let object = Object::try_from(source.lookup(&record.name).unwrap()).unwrap();
                    record.data = Some(
                        source
                            .read_backing_range(object, 0, object.byte_len() as usize)
                            .unwrap(),
                    );
                }
            }
            fixremote::import(
                &destination,
                &transfer.encode().unwrap(),
                &fixremote::Config {
                    policy: policy.clone(),
                    ..fixremote::Config::default()
                },
            )
            .unwrap();
            if let Some(data) = remote_data {
                for path in source.object_paths(data.into(), true).unwrap() {
                    if path.starts_with(".fix/data/") {
                        std::fs::remove_file(source_dir.join(path)).unwrap();
                    }
                }
                assert!(!source.has_backing(data.into()).unwrap());
            }
            let call_label = format!("call-{index}-{case}");
            source.write_label(&call_label, call.into()).unwrap();
            let request: Handle = source
                .create_tree(&[
                    empty,
                    source.read_label("scheduler").unwrap(),
                    mode,
                    collector,
                    input,
                ])
                .unwrap()
                .into();
            if case == 1 {
                source.write_label("bounded-add", input).unwrap();
            }
            let output = run_with_jobs(
                &source_dir,
                &source_agent,
                request,
                None,
                if case == 6 { 0 } else { 1 },
            );
            let diagnostics = String::from_utf8_lossy(&output.stderr).into_owned();
            if case == 6 {
                assert!(output.status.success(), "{diagnostics}");
                let row = diagnostics
                    .lines()
                    .find(|line| line.starts_with("local "))
                    .unwrap();
                let fields: Vec<_> = row.split_whitespace().collect();
                assert_eq!(
                    &fields[1..3],
                    &["0", "0"],
                    "local applications or collector calls ran: {row}"
                );
                assert!(diagnostics.contains("worker: Apply"), "{diagnostics}");
            }
            if case == 1 {
                assert!(
                    diagnostics.lines().any(|line| line.starts_with("worker ")),
                    "{diagnostics}"
                );
                assert!(
                    diagnostics.contains("local: Evaluate bounded-add"),
                    "{diagnostics}"
                );
                assert!(
                    diagnostics.contains("Mem peak") && diagnostics.contains("Received"),
                    "{diagnostics}"
                );
            }
            if case == 0 && index == 0 {
                assert!(diagnostics.contains("worker: Apply"), "{diagnostics}");
            }
            let result = returned(&source, output);
            assert!(
                result.is_tag(),
                "policy {index}, case {case}: {:?}",
                source
                    .read_tree(result)
                    .unwrap()
                    .iter()
                    .map(|value| {
                        Blob::try_from(*value)
                            .ok()
                            .and_then(|blob| source.read_blob(blob).ok())
                            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                    })
                    .collect::<Vec<_>>()
            );
            let claim = source.read_tree(result).unwrap()[1];
            let claim = fixhandle::Ref::try_from(claim).unwrap().object_descriptor();
            let fields = source
                .read_tree(Tree::try_from(Handle::from(claim)).unwrap())
                .unwrap();
            assert_eq!(
                source
                    .read_blob(Blob::try_from(fields[2]).unwrap())
                    .unwrap()
                    .as_ref(),
                &(42 + index * 10 + case).to_le_bytes()
            );
            if case == 0 && index == 0 {
                let info =
                    fixremote::information(&destination, collector, 2, Some(512 << 20)).unwrap();
                assert!(
                    info.results.contains(&fixremote::protocol::Task {
                        operation: 1,
                        collector: *collector.as_bytes(),
                        input: *input.as_bytes()
                    }),
                    "policy {index}, case {case}: root was not delegated"
                );
            }
            let expected_remote = if !(2..5).contains(&case) {
                case >= 5 || (case == 0 && index == 0)
            } else {
                case != 4 && index == 0
            };
            if case != 5 {
                let full_name = call.as_handle().to_string();
                let ran_remotely = diagnostics.lines().any(|line| {
                    let Some((_, title)) = line.split_once("worker: Apply ") else {
                        return false;
                    };
                    let Some(name) = title.split_whitespace().next() else {
                        return false;
                    };
                    let Some((prefix, kind)) = name.rsplit_once('.') else {
                        return false;
                    };
                    full_name.starts_with(prefix) && full_name.ends_with(kind)
                });
                assert_eq!(
                    ran_remotely, expected_remote,
                    "policy {index}, case {case}: {diagnostics}"
                );
            }
        }
    }
}
