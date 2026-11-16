use fixcertificates::Policy;
use fixhandle::{Blob, Handle, Object, Tree};
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
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
        let mut logs = String::new();
        self.logs.read_to_string(&mut logs).unwrap();
        if !logs.is_empty() {
            eprintln!("worker: {logs}");
        }
    }
}

fn configuration(path: &Path, agent: &Agent, compiler: &str, peers: &[String]) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(
        path.join("config"),
        serde_json::to_vec(&serde_json::json!({ "identities": agent.public_keys })).unwrap(),
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
    logs.read_line(&mut line).unwrap();
    let address = line
        .trim()
        .strip_prefix("listening on ")
        .expect(&line)
        .to_string();
    (Worker { child, logs }, address)
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
            fixremote::protocol::send(&mut connection, fixremote::protocol::Message::RequestInfo)
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
}

#[test]
fn remote_commands_are_repository_local_and_trust_commands_are_per_user() {
    let directory = std::env::temp_dir().join(format!("fix-configuration-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let home = directory.join("home");
    std::fs::create_dir(&home).unwrap();
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
    let compiler = Blob::canonical(&[3; 64])
        .unwrap()
        .into_ref()
        .as_handle()
        .to_string();
    command(&home, &["trust", "add-compiler", &compiler], true);
    assert_eq!(
        std::fs::read_to_string(home.join(".fix/config/authorized_keys"))
            .unwrap()
            .trim(),
        agent.public_key
    );
    let listed = command(&home, &["trust", "list"], true);
    assert!(listed.contains(&compiler));
    assert!(listed.contains(&agent.public_key));
    command(&second, &["trust", "remove-key", key], true);
    command(&second, &["trust", "remove-identity", key], true);
    command(&second, &["trust", "remove-compiler", &compiler], true);
    assert!(command(&home, &["trust", "list"], true).is_empty());
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
            .set_remotes(vec![Remote {
                name: "worker".into(),
                address: format!("tcp://{address}"),
            }])
            .unwrap();
        let empty: Handle = Blob::literal(b"").unwrap().into();
        let mode: Handle = Blob::literal(b"eval").unwrap().into();
        let collector = source.read_label("collector").unwrap();
        let function = source.read_label("add").unwrap();
        let advisor = source.read_label("advisor").unwrap();
        for case in 0u64..6 {
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
            let input: Handle = if case < 2 || case == 5 {
                thunk.strict().into()
            } else {
                let output_size = (64u64 << 20).to_le_bytes();
                let output = source
                    .create_blob(if case == 2 { &output_size } else { b"bad" })
                    .unwrap();
                let fan_out: Handle = Blob::literal(&64u64.to_le_bytes()).unwrap().into();
                let parallel: Handle = Blob::literal(&u64::from(case == 4).to_le_bytes())
                    .unwrap()
                    .into();
                let advised = source
                    .create_tree(&[
                        large.into(),
                        advisor,
                        call.into(),
                        output.into(),
                        fan_out,
                        parallel,
                    ])
                    .unwrap();
                advised.into_ref().application().strict().into()
            };
            let bytes = fixremote::export(&source, &[input, collector], &[]).unwrap();
            fixremote::import(
                &destination,
                &bytes,
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
            let result = returned(
                &source,
                run_with_jobs(&source_dir, &source_agent, request, None, 1),
            );
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
            if (case == 0 && index == 0) || case == 5 {
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
            let (operation, target, expected_remote) = if case < 2 || case == 5 {
                (1u64, input, case == 5 || (case == 0 && index == 0))
            } else {
                (4u64, thunk.into(), case != 4 && index == 0)
            };
            let prefix = [
                Blob::literal(&operation.to_le_bytes()).unwrap().into(),
                target,
            ];
            assert_eq!(
                !destination.claims(&prefix).unwrap().is_empty(),
                expected_remote,
                "policy {index}, case {case}"
            );
        }
    }
}
