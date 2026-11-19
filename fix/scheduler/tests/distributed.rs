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
        .arg(format!("fanout={}", fixture(env!("FANOUT")).display()))
        .arg(format!("work={}", fixture(env!("WORK")).display()))
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
            "4096",
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

fn start_worker(
    worker_dir: &Path,
    worker_agent: &Agent,
    jobs: u64,
    memory: u64,
) -> (Worker, String) {
    let mut child = Command::new(fixture(env!("FIX")))
        .args([
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--memory",
            &memory.to_string(),
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
    let (worker, address) = start_worker(&worker_dir, &worker_agent, 1, 2048);

    source
        .set_remotes(vec![Remote {
            name: "worker".into(),
            address: format!("tcp://{address}"),
            scheduler: None,
            collector: None,
        }])
        .unwrap();

    let empty: Handle = source
        .create_blob(&(512u64 << 20).to_le_bytes())
        .unwrap()
        .into();
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

    use fixremote::protocol::{self, Message, Task};
    let mut stream = fixremote::linux::Connection(std::net::TcpStream::connect(&address).unwrap());
    stream
        .0
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
    let keys = &worker_agent.public_keys;
    let input = *combination.as_handle().as_bytes();
    let result = *Blob::literal(&42u64.to_le_bytes())
        .unwrap()
        .as_handle()
        .as_bytes();
    protocol::send(&mut stream, Message::RequestTransfer(input)).unwrap();
    let proposal = protocol::receive(&mut stream, protocol::MAX_MESSAGE_SIZE)
        .unwrap()
        .unwrap();
    assert!(matches!(&proposal, Message::ProposeTransfer { coupons, .. }
        if coupons.iter().any(|coupon| coupon.operation == 2 && coupon.input == input
            && coupon.result == result && coupon.author == *Tree::empty().as_handle().as_bytes())));
    fixremote::transport::complete(protocol::asynchronous::receive_transfer_with_info_from(
        &mut fixremote::transport::Blocking(&mut stream),
        protocol::MAX_MESSAGE_SIZE,
        &protocol::Information::default(),
        keys,
        |offers| fixremote::missing(&source, offers).ok(),
        Some(proposal),
    ))
    .unwrap()
    .unwrap();

    let task = Task {
        operation: 2,
        input,
        collector: *collector.as_bytes(),
    };
    protocol::send(
        &mut stream,
        Message::RequestCoupon {
            task: task.clone(),
            result: *first.as_bytes(),
        },
    )
    .unwrap();
    assert_eq!(
        protocol::receive(&mut stream, protocol::MAX_MESSAGE_SIZE).unwrap(),
        Some(Message::Rejected),
    );
    for task in [
        task,
        Task {
            operation: 1,
            input: *prefix[1].as_bytes(),
            collector: *collector.as_bytes(),
        },
    ] {
        protocol::send(
            &mut stream,
            Message::RequestCoupon {
                task: task.clone(),
                result,
            },
        )
        .unwrap();
        let incoming = protocol::receive_transfer(
            &mut stream,
            protocol::MAX_MESSAGE_SIZE,
            1,
            keys,
            |offers| fixremote::missing(&source, offers).ok(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(incoming.task.as_ref(), Some(&task));
        let roots = fixremote::import(
            &source,
            &incoming.transfer.encode().unwrap(),
            &fixremote::Config {
                policy: source.policy().unwrap(),
                ..fixremote::Config::default()
            },
        )
        .unwrap();
        let coupon = Tree::try_from(roots[0]).unwrap();
        let fields = source.read_tree(coupon).unwrap();
        let claim = fixhandle::TreeRef::try_from(fields[1])
            .unwrap()
            .object_descriptor();
        let claim = source
            .read_tree(Tree::try_from(Handle::from(claim)).unwrap())
            .unwrap();
        let operation: Handle = Blob::literal(&task.operation.to_le_bytes()).unwrap().into();
        assert_eq!(claim[0].as_bytes(), operation.as_bytes());
        assert_eq!(claim[1].as_bytes(), &task.input);
        assert_eq!(claim[2].as_bytes(), &result);
    }
    let reference = combination.into_ref();
    let thunk = reference.application();
    for value in [Handle::from(reference), thunk.into(), thunk.strict().into()] {
        let name = *value.as_bytes();
        protocol::send(&mut stream, Message::RequestTransfer(name)).unwrap();
        let incoming = protocol::receive_transfer(
            &mut stream,
            protocol::MAX_MESSAGE_SIZE,
            1,
            keys,
            |offers| fixremote::missing(&source, offers).ok(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(incoming.result, Some(name));
        assert_eq!(incoming.transfer.roots, vec![name]);
        assert!(
            !incoming
                .transfer
                .records
                .iter()
                .any(|record| record.data.is_some())
        );
    }
    drop(worker);
}

#[test]
fn schedulers_evaluate_with_only_one_resource_eligible_node() {
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
    let (_worker, address) = start_worker(&worker_dir, &worker_agent, 2, 2048);
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
            .with_policy(Policy {
                authorized_keys: vec![
                    source_agent.public_key.clone(),
                    worker_agent.public_key.clone(),
                ],
                ..Policy::default()
            });
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
        let empty: Handle = Blob::literal(&(256u64 << 20).to_le_bytes()).unwrap().into();
        let mode: Handle = Blob::literal(b"eval").unwrap().into();
        let collector = source.read_label("collector").unwrap();
        let function = source.read_label("add").unwrap();
        for (case, budget, jobs) in [
            (0u64, 3u64 << 30, 1),
            (1, 256u64 << 20, 0),
            (2, 5u64 << 30, 1),
        ] {
            let limit = source.create_blob(&budget.to_le_bytes()).unwrap();
            let first: Handle = Blob::literal(&(20 + index * 10 + case).to_le_bytes())
                .unwrap()
                .into();
            let second: Handle = Blob::literal(&22u64.to_le_bytes()).unwrap().into();
            let call = source
                .create_tree(&[limit.into(), function, first, second])
                .unwrap();
            let input: Handle = call.into_ref().application().strict().into();
            let scheduler = source.read_label("scheduler").unwrap();
            let request = source
                .create_tree(&[empty, scheduler, mode, collector, input])
                .unwrap();
            let output = run_with_jobs(&source_dir, &source_agent, request.into(), None, jobs);
            if budget > 4u64 << 30 {
                assert!(!output.status.success());
                let error = String::from_utf8_lossy(&output.stderr);
                assert!(error.contains("memory admission rejected"), "{error}");
                assert!(
                    error.contains(&format!("requested {budget} bytes")),
                    "{error}"
                );
                assert!(error.contains(&Handle::from(call).to_string()), "{error}");
                continue;
            }
            let result = returned(&source, output);
            assert!(result.is_tag());
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
        }
    }
}

#[test]
fn pending_transfer_does_not_block_other_requests_and_links_share_identity() {
    use fixremote::protocol::{self, Message, Packet, topology::LinkId};
    let directory = std::env::temp_dir().join(format!("fix-requests-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let worker_dir = directory.join("worker");
    seed(&worker_dir, env!("PARALLEL"));
    let agent = Agent::new(&directory.join("agent"), 119);
    let compiler = std::fs::read_to_string(worker_dir.join(".fix/trust")).unwrap();
    configuration(&worker_dir.join("user"), &agent, &compiler, &[]);
    sign_seed(&worker_dir, &agent, true);
    let (_worker, address) = start_worker(&worker_dir, &agent, 2, 2048);
    let mut links = Vec::new();
    let mut identity = None;
    for sequence in 0..2 {
        let mut stream =
            fixremote::linux::Connection(std::net::TcpStream::connect(&address).unwrap());
        stream
            .0
            .set_read_timeout(Some(std::time::Duration::from_secs(15)))
            .unwrap();
        protocol::send(
            &mut stream,
            Message::Hello {
                link: LinkId {
                    initiator: [119; 16],
                    sequence,
                },
                node: [119; 16],
            },
        )
        .unwrap();
        let Some(Message::HelloAccepted(node)) =
            protocol::receive(&mut stream, protocol::MAX_MESSAGE_SIZE).unwrap()
        else {
            panic!("missing identity");
        };
        if let Some(previous) = identity {
            assert_eq!(node, previous);
        } else {
            identity = Some(node);
        }
        links.push(stream);
    }
    let stream = &mut links[0];
    fn request(stream: &mut fixremote::linux::Connection, id: u64, message: Message) {
        protocol::send_packet(stream, Packet::Start(id, message)).unwrap();
    }
    fn reply(stream: &mut fixremote::linux::Connection, id: u64) -> Message {
        while let Some(packet) =
            protocol::receive_packet(stream, protocol::MAX_MESSAGE_SIZE).unwrap()
        {
            if let Packet::Job(request, message) = packet
                && request == id
            {
                return message;
            }
        }
        panic!("connection closed before replying to request {id}");
    }
    request(stream, 0, Message::RequestLabels);
    assert!(matches!(reply(stream, 0), Message::ProposeTransfer { .. }));
    request(stream, 2, Message::RequestInfo { inventory: false });
    assert!(matches!(reply(stream, 2), Message::Info(info) if info.parallelism == 2));
    protocol::send_packet(stream, Packet::Stop(0)).unwrap();
    request(stream, 4, Message::RequestInfo { inventory: false });
    assert!(matches!(reply(stream, 4), Message::Info(info) if info.parallelism == 2));
    drop(links.remove(0));
    request(&mut links[0], 0, Message::RequestInfo { inventory: false });
    assert!(matches!(reply(&mut links[0], 0), Message::Info(info) if info.parallelism == 2));
}

#[test]
fn work_returns_through_the_coordinator_and_executes_concurrently() {
    for (index, scheduler) in [env!("ONEPASS"), env!("HINT")].into_iter().enumerate() {
        let directory =
            std::env::temp_dir().join(format!("fix-topology-{}-{index}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let _cleanup = Directory(directory.clone());
        let agent = Agent::new(&directory.join("agent"), 120);
        let coordinator_dir = directory.join("coordinator");
        let small_dir = directory.join("small");
        let large_dir = directory.join("large");
        for path in [&coordinator_dir, &small_dir, &large_dir] {
            seed(path, scheduler);
        }
        let compiler = std::fs::read_to_string(coordinator_dir.join(".fix/trust")).unwrap();
        for path in [&coordinator_dir, &small_dir, &large_dir] {
            configuration(&path.join("user"), &agent, &compiler, &[]);
            sign_seed(path, &agent, true);
        }
        let policy = Policy {
            authorized_keys: agent.public_keys.clone(),
            ..Policy::default()
        };
        let open = |path: &Path| {
            Repository::open(path.join(".fix").to_str().unwrap())
                .unwrap()
                .with_policy(policy.clone())
        };
        let coordinator = open(&coordinator_dir);
        let small = open(&small_dir);
        let large = open(&large_dir);
        let limit: Handle = Blob::literal(&(256u64 << 20).to_le_bytes()).unwrap().into();
        let add = small.read_label("work").unwrap();
        let mut calls: Vec<Handle> = Vec::new();
        for value in 0u64..4 {
            let arguments = [
                limit,
                add,
                Blob::literal(&value.to_le_bytes()).unwrap().into(),
                Blob::literal(&42u64.to_le_bytes()).unwrap().into(),
            ];
            let call = large.create_tree(&arguments).unwrap();
            calls.push(call.into());
        }
        let references: Vec<Handle> = calls
            .iter()
            .map(|call| Tree::try_from(*call).unwrap().into_ref().into())
            .collect();
        let parent_limit: Handle = Blob::literal(&(3u64 << 30).to_le_bytes()).unwrap().into();
        let parent = small
            .create_tree(&[
                parent_limit,
                small.read_label("fanout").unwrap(),
                references[0],
                references[1],
                references[2],
                references[3],
            ])
            .unwrap();
        let input: Handle = parent.into_ref().application().strict().into();
        let collector = coordinator.read_label("collector").unwrap();
        let request: Handle = coordinator
            .create_tree(&[
                limit,
                coordinator.read_label("scheduler").unwrap(),
                Blob::literal(b"eval").unwrap().into(),
                collector,
                input,
            ])
            .unwrap()
            .into();
        let (_small, small_address) = start_worker(&small_dir, &agent, 1, 4096);
        let (_large, large_address) = start_worker(&large_dir, &agent, 4, 2048);
        coordinator
            .set_remotes(vec![
                Remote {
                    name: "small".into(),
                    address: format!("tcp://{small_address}"),
                    scheduler: None,
                    collector: None,
                },
                Remote {
                    name: "large".into(),
                    address: format!("tcp://{large_address}"),
                    scheduler: None,
                    collector: None,
                },
                Remote {
                    name: "large-alias".into(),
                    address: format!("tcp://{large_address}"),
                    scheduler: None,
                    collector: None,
                },
            ])
            .unwrap();
        let output = run_with_jobs(&coordinator_dir, &agent, request, None, 0);
        let diagnostics = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(output.status.success(), "{diagnostics}");
        let result = returned(&coordinator, output);
        assert!(result.is_tag(), "{diagnostics}");
        let row = diagnostics
            .lines()
            .find(|line| {
                line.split_whitespace().next().is_some_and(|name| {
                    matches!(name.rsplit('!').next(), Some("large" | "large-alias"))
                })
            })
            .expect(&diagnostics);
        let fields: Vec<_> = row.split_whitespace().collect();
        assert!(
            fields[4].split('/').next().unwrap().parse::<u64>().unwrap() > 1,
            "large worker did not execute applications concurrently: {diagnostics}"
        );
        let claim = coordinator.read_tree(result).unwrap()[1];
        let claim = fixhandle::Ref::try_from(claim).unwrap().object_descriptor();
        let result = coordinator
            .read_tree(Tree::try_from(Handle::from(claim)).unwrap())
            .unwrap()[2];
        let results = coordinator
            .read_tree(Tree::try_from(result).unwrap())
            .unwrap();
        for (index, result) in results.iter().enumerate() {
            let mut expected = index as u64 + 42;
            for _ in 0..20_000_000 {
                expected = expected.wrapping_mul(1664525).wrapping_add(1013904223);
            }
            assert_eq!(
                coordinator
                    .read_blob(Blob::try_from(*result).unwrap())
                    .unwrap()
                    .as_ref(),
                expected.to_le_bytes()
            );
        }
    }
}

#[test]
fn locally_placed_applications_use_remote_memory_when_admission_is_blocked() {
    for (index, scheduler) in [env!("ONEPASS"), env!("HINT")].into_iter().enumerate() {
        let directory =
            std::env::temp_dir().join(format!("fix-memory-spill-{}-{index}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let _cleanup = Directory(directory.clone());
        let agent = Agent::new(&directory.join("agent"), 120);
        let local_dir = directory.join("local");
        let remote_dir = directory.join("remote");
        let local = seed(&local_dir, scheduler);
        seed(&remote_dir, scheduler);
        let compiler = std::fs::read_to_string(local_dir.join(".fix/trust")).unwrap();
        for path in [&local_dir, &remote_dir] {
            configuration(&path.join("user"), &agent, &compiler, &[]);
            sign_seed(path, &agent, true);
        }
        let local = local.with_policy(Policy {
            authorized_keys: agent.public_keys.clone(),
            ..Policy::default()
        });
        let blob = |value: u64| -> Handle { Blob::literal(&value.to_le_bytes()).unwrap().into() };
        let work = local.read_label("work").unwrap();
        let calls: Vec<Handle> = (0..4)
            .map(|value| {
                local
                    .create_tree(&[blob(3 << 30), work, blob(value), blob(42)])
                    .unwrap()
                    .into_ref()
                    .application()
                    .strict()
                    .into()
            })
            .collect();
        let request = local
            .create_tree(&[
                blob(256 << 20),
                local.read_label("scheduler").unwrap(),
                Blob::literal(b"eval").unwrap().into(),
                local.read_label("collector").unwrap(),
                local.create_tree(&calls).unwrap().into(),
            ])
            .unwrap()
            .into();
        let (_remote, address) = start_worker(&remote_dir, &agent, 2, 4096);
        let output = run(
            &local_dir,
            &agent,
            request,
            Some(&format!("spare=tcp://{address}")),
        );
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{diagnostics}");
        for worker in ["local", "spare"] {
            assert!(
                diagnostics.lines().any(|line| {
                    let mut fields = line.split_whitespace();
                    fields.next() == Some(worker)
                        && fields
                            .next()
                            .and_then(|field| field.parse::<u64>().ok())
                            .is_some_and(|count| count > 0)
                }),
                "{worker} executed no applications: {diagnostics}"
            );
        }
    }
}

#[test]
fn an_idle_worker_opens_configured_links_when_it_joins() {
    use fixremote::protocol::{
        self, Message, Packet,
        topology::{LinkId, Topology},
    };
    let directory =
        std::env::temp_dir().join(format!("fix-idle-peer-links-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let _cleanup = Directory(directory.clone());
    let agent = Agent::new(&directory.join("agent"), 121);
    let idle_dir = directory.join("idle");
    let peer_dir = directory.join("peer");
    let idle = seed(&idle_dir, env!("HINT"));
    seed(&peer_dir, env!("HINT"));
    let compiler = std::fs::read_to_string(idle_dir.join(".fix/trust")).unwrap();
    for path in [&idle_dir, &peer_dir] {
        configuration(&path.join("user"), &agent, &compiler, &[]);
        sign_seed(path, &agent, true);
    }
    let (_peer, peer_address) = start_worker(&peer_dir, &agent, 2, 2048);
    idle.set_remotes(vec![Remote {
        name: "peer".into(),
        address: format!("tcp://{peer_address}"),
        scheduler: None,
        collector: None,
    }])
    .unwrap();
    let (_idle, idle_address) = start_worker(&idle_dir, &agent, 2, 2048);
    let mut stream =
        fixremote::linux::Connection(std::net::TcpStream::connect(idle_address).unwrap());
    stream
        .0
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let initiator = [121; 16];
    protocol::send(
        &mut stream,
        Message::Hello {
            link: LinkId {
                initiator,
                sequence: 0,
            },
            node: initiator,
        },
    )
    .unwrap();
    let Some(Message::HelloAccepted(idle)) =
        protocol::receive(&mut stream, protocol::MAX_MESSAGE_SIZE).unwrap()
    else {
        panic!("idle worker did not join");
    };
    let mut topology = Topology::default();
    while let Some(packet) =
        protocol::receive_packet(&mut stream, protocol::MAX_MESSAGE_SIZE).unwrap()
    {
        if let Packet::Topology(update) = packet {
            topology.merge(&update);
            if topology
                .routes(idle)
                .iter()
                .any(|route| route.name == "peer")
            {
                return;
            }
        }
    }
    panic!("idle worker did not open its configured peer link");
}
