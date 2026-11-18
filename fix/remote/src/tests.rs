use super::*;
use alloc::{format, vec};
use core::sync::atomic::{AtomicU64, Ordering};
use fixhandle::{Blob, Tree};
use ssh_key::{HashAlg, PrivateKey, private::Ed25519Keypair};
use std::path::PathBuf;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fix-remote-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn repository(&self) -> Repository {
        Repository::init(self.0.join(".fix").to_str().unwrap())
            .unwrap()
            .with_policy(config().policy)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn key() -> PrivateKey {
    PrivateKey::from(Ed25519Keypair::from_seed(&[7; 32]))
}
fn config() -> Config {
    Config {
        policy: Policy {
            authorized_keys: vec![key().public_key().to_openssh().unwrap()],
            ..Policy::default()
        },
        ..Config::default()
    }
}

fn signer(_: &str, namespace: &str, message: &[u8]) -> Result<String, fixcertificates::Error> {
    Ok(key()
        .sign(namespace, HashAlg::Sha512, message)?
        .to_pem(ssh_key::LineEnding::LF)?)
}
fn certificate(name: [u8; 32]) -> Vec<u8> {
    Certificate {
        name,
        signature: signer("", fixcertificates::TAG_NAMESPACE, &name).unwrap(),
    }
    .encode()
    .unwrap()
}
fn transfer(
    repository: &Repository,
    roots: &[Handle],
    omitted: &[Object],
) -> Result<Vec<u8>, Error> {
    export(repository, roots, omitted)
}

#[test]
fn signed_coupons_keep_their_authors_and_claims_between_repositories() {
    let source = Directory::new();
    let source = source.repository();
    let destination = Directory::new();
    let destination = destination.repository();
    let data = source.create_blob(&[17; 4096]).unwrap();
    let claim = source
        .create_tree(&[
            Blob::literal(&1u64.to_le_bytes()).unwrap().into(),
            data.into_ref().into(),
            data.into(),
        ])
        .unwrap();
    let author = source.create_blob(&[41; 128]).unwrap();
    let fields = [author.into(), claim.into_ref().into()];
    let tag = Tree::canonical(&fields).unwrap().tag_descriptor();
    source
        .publish_certified(
            tag.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
            Some(&certificate(*tag.as_handle().as_bytes())),
        )
        .unwrap();
    let parent = source.create_tree(&[tag.into(), data.into()]).unwrap();
    let bytes = transfer(&source, &[parent.into()], &[]).unwrap();
    let imported = import(&destination, &bytes, &config()).unwrap();
    assert!(imported[0].same_encoding(&parent.into()));
    let fields = destination.read_tree(parent).unwrap();
    assert!(fields[0].same_encoding(&tag.into()));
    assert!(destination.read_tree(tag).unwrap()[0].same_encoding(&author.into()));
    assert!(!destination.has_backing(author.object()).unwrap());
    destination.write_label("coupon", tag.into()).unwrap();
    assert_eq!(destination.read_blob(data).unwrap().as_ref(), &[17; 4096]);
    assert_eq!(
        destination
            .claims(&[Blob::literal(&1u64.to_le_bytes()).unwrap().into()])
            .unwrap()
            .len(),
        1
    );
    import(&destination, &bytes, &config()).unwrap();
}

#[test]
fn thin_references_transfer_todo_metadata_without_fetching_backing() {
    let source = Directory::new();
    let source = source.repository();
    let destination = Directory::new();
    let destination = destination.repository();
    let data = source.create_blob(&[23; 4096]).unwrap();
    let todo = source
        .create_blob(b"scheduler-specific acquisition instructions")
        .unwrap();
    source.set_todo(data.into_ref(), todo.into()).unwrap();
    let bytes = transfer(&source, &[data.into_ref().into()], &[data.object()]).unwrap();
    let imported = import(&destination, &bytes, &config()).unwrap();
    assert!(imported[0].same_encoding(&data.into_ref().into()));
    assert!(!destination.has_backing(data.object()).unwrap());
    assert!(
        !information(&destination, Tree::empty().into(), 1, None)
            .unwrap()
            .data
            .contains(data.as_handle().as_bytes())
    );
    assert!(
        destination
            .get_todo(data.into_ref())
            .unwrap()
            .unwrap()
            .same_encoding(&todo.into())
    );
    let bytes = transfer(&source, &[data.into()], &[]).unwrap();
    import(&destination, &bytes, &config()).unwrap();
    assert_eq!(destination.read_blob(data).unwrap().as_ref(), &[23; 4096]);
    assert!(
        information(&destination, Tree::empty().into(), 1, None)
            .unwrap()
            .data
            .contains(data.as_handle().as_bytes())
    );
}

#[test]
fn thin_object_transfer_can_be_imported_as_a_reference() {
    let source = Directory::new();
    let source = source.repository();
    let destination = Directory::new();
    let destination = destination.repository();
    let blob = source.create_blob(&[17; 128]).unwrap();
    let bytes = transfer(&source, &[blob.into()], &[blob.object()]).unwrap();
    let mut transfer = Transfer::decode(&bytes).unwrap();
    transfer.roots[0][31] |= 0x10;
    let roots = import(&destination, &transfer.encode().unwrap(), &config()).unwrap();
    assert!(roots[0].same_encoding(&blob.into_ref().into()));
    assert!(!destination.has_backing(blob.object()).unwrap());
}

#[test]
fn runnable_transfer_keeps_its_author_reference_thin() {
    let source = Directory::new();
    let source = source.repository();
    let destination = Directory::new();
    let destination = destination.repository();
    let author = source.create_blob(&[11; 4096]).unwrap();
    let executable = source.create_blob(&[17; 128]).unwrap();
    let fields = [
        author.into_ref().into(),
        executable.into(),
        Blob::literal(b"Runnable").unwrap().into(),
    ];
    let runnable = Tree::canonical(&fields).unwrap().tag_descriptor();
    source
        .publish_certified(
            runnable.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
            Some(&certificate(*runnable.as_handle().as_bytes())),
        )
        .unwrap();
    let bytes = transfer(&source, &[runnable.into()], &[]).unwrap();
    let transfer = Transfer::decode(&bytes).unwrap();
    assert!(transfer.records.iter().any(|record| {
        record.name == *executable.as_handle().as_bytes() && record.data.is_some()
    }));
    import(&destination, &bytes, &config()).unwrap();
    destination
        .write_label("runnable", runnable.into())
        .unwrap();
    assert!(!destination.has_backing(author.object()).unwrap());
    assert!(destination.has_backing(executable.object()).unwrap());
}

#[test]
fn pulling_a_tree_includes_coupons_for_its_children() {
    let source = Directory::new();
    let source = source.repository();
    let destination = Directory::new();
    let destination = destination.repository();
    let input = source.create_blob(&[21; 128]).unwrap();
    let result = source.create_blob(&[34; 256]).unwrap();
    let claim = source
        .create_tree(&[
            Blob::literal(&1u64.to_le_bytes()).unwrap().into(),
            input.into(),
            result.into(),
        ])
        .unwrap();
    let fields = [Tree::empty().into(), claim.into_ref().into()];
    let coupon = Tree::canonical(&fields).unwrap().tag_descriptor();
    source
        .publish_certified(
            coupon.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
            Some(&certificate(*coupon.as_handle().as_bytes())),
        )
        .unwrap();
    source.index_tag(coupon).unwrap();
    let inner = source.create_tree(&[input.into()]).unwrap();
    let root = source.create_tree(&[inner.into()]).unwrap();
    let bytes = export_with_coupons(&source, &[root.into()], &[]).unwrap();
    let imported = import(&destination, &bytes, &config()).unwrap();
    assert_eq!(imported.len(), 1);
    assert!(imported[0].same_encoding(&root.into()));
    assert_eq!(destination.read_blob(result).unwrap().as_ref(), &[34; 256]);
    let claims = destination
        .claims(&[
            Blob::literal(&1u64.to_le_bytes()).unwrap().into(),
            input.into(),
        ])
        .unwrap();
    assert_eq!(claims.len(), 1);
    assert!(claims[0].as_handle().same_encoding(coupon.as_handle()));
}

#[test]
fn thin_tag_certificates_retain_authority_without_backing() {
    let source = Directory::new();
    let source = source.repository();
    let directory = Directory::new();
    let destination = directory.repository();
    let fields = [
        Tree::empty().into(),
        Blob::literal(b"claim").unwrap().into(),
    ];
    let tag = Tree::canonical(&fields).unwrap().tag_descriptor();
    source
        .publish_certified(
            tag.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
            Some(&certificate(*tag.as_handle().as_bytes())),
        )
        .unwrap();
    let bytes = transfer(&source, &[tag.into_ref().into()], &[tag.object()]).unwrap();
    let roots = import(&destination, &bytes, &config()).unwrap();
    assert!(roots[0].same_encoding(&tag.into_ref().into()));
    assert!(!destination.has_backing(tag.object()).unwrap());
    destination
        .inspect_tags(&[*tag.into_ref().as_handle().as_bytes()])
        .unwrap();
    let bytes = transfer(&source, &[tag.into()], &[]).unwrap();
    import(&destination, &bytes, &config()).unwrap();
    let reopened = Repository::open(destination.path())
        .unwrap()
        .with_policy(config().policy);
    assert!(reopened.read_tree(tag).unwrap()[0].same_encoding(&Tree::empty().into()));
}

#[test]
fn content_addressed_data_and_certificate_blobs_are_checked_before_admission() {
    let directory = Directory::new();
    let source = directory.repository();
    let directory = Directory::new();
    let destination = directory.repository();
    let data = source.create_blob(&[29; 64]).unwrap();
    let bytes = transfer(&source, &[data.into()], &[]).unwrap();
    import(&destination, &bytes, &Config::default()).unwrap();
    let mut bundle = Transfer::decode(&bytes).unwrap();
    bundle.records[0].data = Some(vec![30; 64]);
    assert!(import(&destination, &bundle.encode().unwrap(), &config()).is_err());

    let fields = [Tree::empty().into(), data.into()];
    let tag = Tree::canonical(&fields).unwrap().tag_descriptor();
    source
        .publish_certified(
            tag.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
            Some(&certificate(*tag.as_handle().as_bytes())),
        )
        .unwrap();
    let bytes = transfer(&source, &[tag.into()], &[]).unwrap();
    assert!(matches!(
        import(&destination, &bytes, &Config::default()),
        Err(Error::Untrusted)
    ));
    let mut bundle = Transfer::decode(&bytes).unwrap();
    let record = bundle
        .records
        .iter()
        .find(|record| record.name == *tag.as_handle().as_bytes())
        .unwrap();
    let certificate_name = record.certificates[0].blob;
    let record = bundle
        .certificates
        .iter_mut()
        .find(|record| record.name == certificate_name)
        .unwrap();
    record.data.as_mut().unwrap()[0] ^= 1;
    assert!(import(&destination, &bundle.encode().unwrap(), &config()).is_err());
    assert!(destination.lookup(tag.as_handle().as_bytes()).is_err());
}

#[test]
fn tree_dependencies_and_certificate_namespaces_are_validated() {
    let source_directory = Directory::new();
    let source = source_directory.repository();
    let destination_directory = Directory::new();
    let destination = destination_directory.repository();
    let data = source.create_blob(&[43; 64]).unwrap();
    let tree = source.create_tree(&[data.into()]).unwrap();
    let bytes = transfer(&source, &[tree.into()], &[]).unwrap();
    let mut bundle = Transfer::decode(&bytes).unwrap();
    bundle
        .records
        .retain(|record| record.name != *data.as_handle().as_bytes());
    assert!(import(&destination, &bundle.encode().unwrap(), &config()).is_err());
    assert!(destination.lookup(tree.as_handle().as_bytes()).is_err());

    let fields = [Tree::empty().into(), tree.into()];
    let tag = Tree::canonical(&fields).unwrap().tag_descriptor();
    source
        .publish_certified(
            tag.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
            Some(&certificate(*tag.as_handle().as_bytes())),
        )
        .unwrap();
    let bytes = transfer(&source, &[tag.into()], &[]).unwrap();
    let mut bundle = Transfer::decode(&bytes).unwrap();
    let wrong = Certificate {
        name: *tag.as_handle().as_bytes(),
        signature: key()
            .sign(
                "another-application",
                HashAlg::Sha512,
                tag.as_handle().as_bytes(),
            )
            .unwrap()
            .to_pem(ssh_key::LineEnding::LF)
            .unwrap(),
    }
    .encode()
    .unwrap();
    let blob = Blob::canonical(&wrong).unwrap();
    let record = bundle
        .records
        .iter_mut()
        .find(|record| record.name == *tag.as_handle().as_bytes())
        .unwrap();
    let old = record.certificates[0].blob;
    record.certificates[0].blob = *blob.as_handle().as_bytes();
    bundle.certificates.retain(|record| record.name != old);
    bundle.certificates.push(Record {
        name: *blob.as_handle().as_bytes(),
        data: Some(wrong),
        todo: None,
        certificates: Vec::new(),
    });
    assert!(matches!(
        import(&destination, &bundle.encode().unwrap(), &config()),
        Err(Error::Transfer(_))
    ));
}

#[test]
fn omitted_backing_requires_references_or_previously_admitted_objects() {
    let source = Directory::new();
    let source = source.repository();
    let destination = Directory::new();
    let destination = destination.repository();
    let object = source.create_blob(&[53; 64]).unwrap();
    let thin = transfer(&source, &[object.into()], &[object.object()]).unwrap();
    let error = import(&destination, &thin, &config()).unwrap_err();
    let message = format!("{error}");
    assert!(message.contains("backing for Object"));
    assert!(message.contains(&format!("{}", object.as_handle())));
    assert!(message.contains(destination.path()));
    let full = transfer(&source, &[object.into()], &[]).unwrap();
    import(&destination, &full, &config()).unwrap();
    let values = import(&destination, &thin, &config()).unwrap();
    assert!(values[0].same_encoding(&object.into()));
}

#[test]
fn framed_transfers_round_trip_over_tcp_and_reject_excessive_lengths() {
    use crate::linux::Connection;
    use std::io::Write;
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut connection = Connection(listener.accept().unwrap().0);
        let bytes = transport::receive(&mut connection, 1 << 20)
            .unwrap()
            .unwrap();
        transport::send(&mut connection, &bytes).unwrap();
        connection
            .0
            .write_all(&((1u64 << 20) + 1).to_be_bytes())
            .unwrap();
    });
    let mut connection = Connection::connect(address).unwrap();
    let bytes = vec![71; 1 << 18];
    transport::send(&mut connection, &bytes).unwrap();
    let mut progress = Vec::new();
    let received = transport::complete(transport::asynchronous::receive_observed(
        &mut transport::Blocking(&mut connection),
        1 << 20,
        &mut |completed, total| progress.push((completed, total)),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(received, bytes);
    assert_eq!(progress.first(), Some(&(0, bytes.len() as u64)));
    assert_eq!(
        progress.last(),
        Some(&(bytes.len() as u64, bytes.len() as u64))
    );
    assert!(
        progress
            .windows(2)
            .all(|pair| pair[0].0 < pair[1].0 && pair[1].0 - pair[0].0 <= 65536)
    );
    assert!(matches!(
        transport::receive(&mut connection, 1 << 20),
        Err(transport::Error::TooLarge { length, limit })
            if length == (1 << 20) + 1 && limit == 1 << 20
    ));
    server.join().unwrap();
}

#[test]
fn ssh_agent_signs_tag_certificates_and_refuses_unknown_identities() {
    let directory = Directory::new();
    let agent = fixremote_testing::Agent::new(&directory.0.join("agent"), 7);
    let repository = directory.repository().unsigned();
    let data = repository.create_blob(&[37; 4096]).unwrap();
    let fields = [Tree::empty().into(), data.into()];
    let object = Tree::canonical(&fields).unwrap().tag_descriptor();
    repository
        .publish_certified(
            object.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
            None,
        )
        .unwrap();
    let (_, blob) = certify(
        &repository,
        *object.as_handle().as_bytes(),
        &[agent.public_key.clone()],
        &[agent.public_key.clone()],
        |identity, namespace, message| {
            linux::sign_with_agent(&agent.socket, identity, namespace, message)
                .map_err(|error| fixcertificates::Error::Signing(format!("{error}")))
        },
    )
    .unwrap();
    assert!(
        Certificate::decode(blob.data.as_ref().unwrap())
            .unwrap()
            .verify(object.as_handle().as_bytes(), &config().policy)
            .is_ok()
    );
    let other = PrivateKey::from(Ed25519Keypair::from_seed(&[8; 32]));
    let error = linux::sign_with_agent(
        &agent.socket,
        &other.public_key().to_openssh().unwrap(),
        fixcertificates::TAG_NAMESPACE,
        b"message",
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}

#[test]
fn negotiated_transfers_include_certificate_blobs_and_reuse_existing_objects() {
    use crate::{
        linux::Connection,
        protocol::{self, Message, Task},
    };
    let source_directory = Directory::new();
    let source = source_directory.repository();
    let destination_directory = Directory::new();
    let destination = destination_directory.repository();
    let data = source.create_blob(&[89; 65536]).unwrap();
    let fields = [Tree::empty().into(), data.into()];
    let tag = Tree::canonical(&fields).unwrap().tag_descriptor();
    source
        .publish_certified(
            tag.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
            Some(&certificate(*tag.as_handle().as_bytes())),
        )
        .unwrap();
    let task = Task {
        operation: 1,
        collector: *Tree::empty().as_handle().as_bytes(),
        input: *tag.as_handle().as_bytes(),
    };
    let encoded = export(&source, &[Tree::empty().into(), tag.into()], &[]).unwrap();
    let transfer = Transfer::decode(&encoded).unwrap();
    let certificate_name = transfer
        .records
        .iter()
        .find(|record| record.name == task.input)
        .unwrap()
        .certificates[0]
        .blob;
    assert!(
        transfer
            .certificates
            .iter()
            .any(|record| record.name == certificate_name && record.data.is_some())
    );
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut connection = Connection(listener.accept().unwrap().0);
        let mut counts = Vec::new();
        for _ in 0..2 {
            let received = protocol::receive_transfer(
                &mut connection,
                1 << 20,
                3,
                &trusted_keys(&config()).unwrap(),
                |offers| {
                    assert!(!offers.iter().any(|offer| offer.name == certificate_name));
                    missing(&destination, offers).ok()
                },
            )
            .unwrap()
            .unwrap();
            assert!(received.result.is_none());
            counts.push(
                received
                    .transfer
                    .records
                    .iter()
                    .filter(|record| record.data.is_some())
                    .count(),
            );
            import(
                &destination,
                &received.transfer.encode().unwrap(),
                &config(),
            )
            .unwrap();
            let blob = destination
                .certificate_blobs(tag.as_handle().as_bytes())
                .unwrap()[0];
            assert_eq!(*blob.as_handle().as_bytes(), certificate_name);
            assert_eq!(destination.read_blob(data).unwrap().as_ref(), &[89; 65536]);
            let reply =
                Transfer::decode(&export(&destination, &[tag.into()], &[]).unwrap()).unwrap();
            protocol::send_transfer(
                &mut connection,
                1 << 20,
                received.task.as_ref().unwrap(),
                Some(*tag.as_handle().as_bytes()),
                reply,
                |_, _| None,
            )
            .unwrap();
        }
        assert!(
            protocol::receive_transfer(
                &mut connection,
                1 << 20,
                3,
                &trusted_keys(&config()).unwrap(),
                |_| unreachable!()
            )
            .unwrap()
            .is_none()
        );
        counts
    });
    let mut connection = Connection::connect(address).unwrap();
    protocol::send(&mut connection, Message::RequestInfo { inventory: true }).unwrap();
    assert!(matches!(
        protocol::receive(&mut connection, 1 << 20).unwrap(),
        Some(Message::Info(protocol::Information { parallelism: 3, .. }))
    ));
    for _ in 0..2 {
        protocol::send_transfer(
            &mut connection,
            1 << 20,
            &task,
            None,
            transfer.clone(),
            |_, _| None,
        )
        .unwrap();
        let reply = protocol::receive_transfer(
            &mut connection,
            1 << 20,
            1,
            &trusted_keys(&config()).unwrap(),
            |offers| missing(&source, offers).ok(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(reply.task, Some(task.clone()));
        assert_eq!(reply.result, Some(*tag.as_handle().as_bytes()));
        assert!(
            reply
                .transfer
                .records
                .iter()
                .all(|record| record.data.is_none())
        );
        import(&source, &reply.transfer.encode().unwrap(), &config()).unwrap();
    }
    drop(connection);
    let counts = server.join().unwrap();
    assert!(counts[0] >= 3);
    assert_eq!(counts[1], 0);
}

#[test]
fn negotiated_transfers_reject_unsolicited_data_and_mismatched_tasks() {
    use crate::{
        linux::Connection,
        protocol::{self, Message, Offer, Task},
    };
    let name = *Blob::canonical(&[97; 64]).unwrap().as_handle().as_bytes();
    let task = Task {
        operation: 1,
        collector: *Tree::empty().as_handle().as_bytes(),
        input: name,
    };
    for unsolicited in [true, false] {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let expected = task.clone();
        let server = std::thread::spawn(move || {
            let mut connection = Connection(listener.accept().unwrap().0);
            assert!(matches!(
                protocol::receive_transfer(
                    &mut connection,
                    1 << 20,
                    1,
                    &trusted_keys(&config()).unwrap(),
                    |_| Some(vec![name])
                ),
                Err(protocol::Error::Invalid)
            ));
        });
        let mut connection = Connection::connect(address).unwrap();
        if unsolicited {
            protocol::send(
                &mut connection,
                Message::BlobData(Record {
                    name,
                    data: Some(vec![97; 64]),
                    todo: None,
                    certificates: Vec::new(),
                }),
            )
            .unwrap();
        } else {
            protocol::send(
                &mut connection,
                Message::ProposeTransfer {
                    task: Some(expected.clone()),
                    result: None,
                    objects: vec![Offer {
                        name,
                        backing: true,
                        todo: None,
                    }],
                },
            )
            .unwrap();
            assert!(matches!(
                protocol::receive(&mut connection, 1 << 20).unwrap(),
                Some(Message::AcceptTransfer { .. })
            ));
            let mut different = expected;
            different.operation = 4;
            protocol::send(&mut connection, Message::Run(different)).unwrap();
        }
        server.join().unwrap();
    }
}

#[test]
fn scheduler_staging_negotiates_missing_program_data() {
    use crate::{
        linux::Connection,
        protocol::{self, Message, Task, Transfer},
    };
    let source = Directory::new();
    let source = source.repository();
    let collector = source.create_blob(&[9; 64]).unwrap();
    let scheduler = source.create_blob(&[42; 4096]).unwrap();
    let task = Task::stage_programs(
        *collector.as_handle().as_bytes(),
        *scheduler.as_handle().as_bytes(),
    );
    let transfer =
        Transfer::decode(&export(&source, &[collector.into(), scheduler.into()], &[]).unwrap())
            .unwrap();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let expected = task.clone();
    let server = std::thread::spawn(move || {
        let mut connection = Connection(listener.accept().unwrap().0);
        let incoming = protocol::receive_transfer(&mut connection, 1 << 20, 1, &[], |offers| {
            assert!(offers.iter().any(|offer| offer.name == expected.collector));
            assert!(offers.iter().any(|offer| offer.name == expected.input));
            Some(vec![expected.input])
        })
        .unwrap()
        .unwrap();
        assert_eq!(incoming.task, Some(expected.clone()));
        assert_eq!(
            incoming.transfer.roots,
            vec![expected.collector, expected.input]
        );
        assert!(incoming.transfer.records.iter().any(|record| {
            record.name == expected.input && record.data.as_deref() == Some(&[42; 4096][..])
        }));
        assert!(
            incoming
                .transfer
                .records
                .iter()
                .any(|record| { record.name == expected.collector && record.data.is_none() })
        );
        protocol::send(&mut connection, Message::ObjectReceived(expected.input)).unwrap();
    });
    let mut connection = Connection::connect(address).unwrap();
    protocol::send_transfer(&mut connection, 1 << 20, &task, None, transfer, |_, _| None).unwrap();
    assert!(
        matches!(protocol::receive(&mut connection, 1 << 20).unwrap(), Some(Message::ObjectReceived(name)) if name == task.input)
    );
    server.join().unwrap();
}

#[test]
fn protocol_frames_reject_unknown_versions_trailing_data_and_truncation() {
    use crate::{
        linux::Connection,
        protocol::{self, Message},
    };
    use std::io::Write;
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut connection = Connection(listener.accept().unwrap().0);
        let mut unsupported = postcard::to_allocvec(&(protocol::VERSION + 1)).unwrap();
        unsupported.push(255);
        transport::send(&mut connection, &unsupported).unwrap();
        let mut trailing =
            postcard::to_allocvec(&(protocol::VERSION, Message::RequestInfo { inventory: false }))
                .unwrap();
        trailing.push(0);
        transport::send(&mut connection, &trailing).unwrap();
        let mut malformed = postcard::to_allocvec(&protocol::VERSION).unwrap();
        malformed.extend([255, 255, 255, 255, 255]);
        transport::send(&mut connection, &malformed).unwrap();
        connection.0.write_all(&[0; 3]).unwrap();
    });
    let mut connection = Connection::connect(address).unwrap();
    assert!(matches!(
        protocol::receive(&mut connection, 1024),
        Err(protocol::Error::Version { expected, received })
            if expected == protocol::VERSION && received == protocol::VERSION + 1
    ));
    assert!(matches!(
        protocol::receive(&mut connection, 1024),
        Err(protocol::Error::Invalid)
    ));
    let error = protocol::receive(&mut connection, 1024).unwrap_err();
    assert!(matches!(error, protocol::Error::Decode { .. }), "{error}");
    assert!(format!("{error}").contains("could not decode protocol"));
    assert!(matches!(
        protocol::receive(&mut connection, 1024),
        Err(protocol::Error::Transport(transport::Error::Truncated))
    ));
    server.join().unwrap();
}

#[test]
fn a_missing_unsigned_tag_is_signed_on_demand_with_a_key_the_receiver_trusts() {
    use crate::{
        linux::Connection,
        protocol::{self, Task},
    };
    let directory = Directory::new();
    let staging = directory.repository().unsigned();
    let fields = [
        Tree::empty().into(),
        Blob::literal(b"unsigned claim").unwrap().into(),
    ];
    let tag = Tree::canonical(&fields).unwrap().tag_descriptor();
    staging
        .publish(
            tag.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let agent = fixremote_testing::Agent::with_keys(&directory.0.join("agent"), &[61, 62]);
    let source = Repository::open(staging.path())
        .unwrap()
        .with_policy(Policy {
            identities: agent.public_keys.clone(),
            authorized_keys: agent.public_keys.clone(),
            ..Policy::default()
        });
    assert!(source.lookup(tag.as_handle().as_bytes()).is_err());
    let transfer =
        Transfer::decode(&export(&source, &[Tree::empty().into(), tag.into()], &[]).unwrap())
            .unwrap();
    assert!(transfer.certificates.is_empty());
    let destination_directory = Directory::new();
    let destination_config = Config {
        policy: Policy {
            authorized_keys: vec![agent.public_keys[1].clone()],
            ..Policy::default()
        },
        ..Config::default()
    };
    let destination = Repository::init(destination_directory.0.join(".fix").to_str().unwrap())
        .unwrap()
        .with_policy(destination_config.policy.clone());
    let trusted = trusted_keys(&destination_config).unwrap();
    let task = Task {
        operation: 1,
        collector: *Tree::empty().as_handle().as_bytes(),
        input: *tag.as_handle().as_bytes(),
    };
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut connection = Connection(listener.accept().unwrap().0);
        for round in 0..2 {
            let incoming =
                protocol::receive_transfer(&mut connection, 1 << 20, 1, &trusted, |offers| {
                    missing(&destination, offers).ok()
                })
                .unwrap()
                .unwrap();
            if round == 0 {
                let record = incoming
                    .transfer
                    .records
                    .iter()
                    .find(|record| record.name == *tag.as_handle().as_bytes())
                    .unwrap();
                assert_eq!(record.certificates.len(), 1);
                assert_eq!(record.certificates[0].key, trusted[0]);
            } else {
                assert!(
                    incoming
                        .transfer
                        .records
                        .iter()
                        .all(|record| record.data.is_none())
                );
            }
            import(
                &destination,
                &incoming.transfer.encode().unwrap(),
                &destination_config,
            )
            .unwrap();
            let certificates = destination.tag_certificates(tag.into()).unwrap();
            assert_eq!(certificates.len(), 1);
            assert_eq!(certificates[0].signer().unwrap(), trusted[0]);
        }
    });
    let mut connection = Connection::connect(address).unwrap();
    let mut requests = 0;
    let mut signatures = 0;
    for _ in 0..2 {
        protocol::send_transfer(
            &mut connection,
            1 << 20,
            &task,
            None,
            transfer.clone(),
            |name, keys| {
                requests += 1;
                certify(
                    &source,
                    name,
                    keys,
                    &agent.public_keys,
                    |identity, namespace, message| {
                        signatures += 1;
                        assert_eq!(identity, &agent.public_keys[1]);
                        linux::sign_with_agent(&agent.socket, identity, namespace, message)
                            .map_err(|error| fixcertificates::Error::Signing(format!("{error}")))
                    },
                )
                .ok()
            },
        )
        .unwrap();
    }
    server.join().unwrap();
    assert_eq!(requests, 1);
    assert_eq!(signatures, 1);
    assert_eq!(source.tag_certificates(tag.into()).unwrap().len(), 1);
    let (_, _) = certify(
        &source,
        *tag.as_handle().as_bytes(),
        &agent.public_keys,
        &[],
        |_, _, _| panic!("reuse requires no signing agent"),
    )
    .unwrap();
    certify(
        &source,
        *tag.as_handle().as_bytes(),
        &agent.public_keys[..1],
        &agent.public_keys,
        |identity, namespace, message| {
            linux::sign_with_agent(&agent.socket, identity, namespace, message)
                .map_err(|error| fixcertificates::Error::Signing(format!("{error}")))
        },
    )
    .unwrap();
    let certificates = source.tag_certificates(tag.into()).unwrap();
    assert_eq!(certificates.len(), 2);
    assert_eq!(
        source
            .certificate_blobs(tag.as_handle().as_bytes())
            .unwrap()
            .len(),
        2
    );
    for key in &agent.public_keys {
        assert!(certificates.iter().any(|certificate| {
            certificate
                .verify(
                    tag.as_handle().as_bytes(),
                    &Policy {
                        authorized_keys: vec![key.clone()],
                        ..Policy::default()
                    },
                )
                .is_ok()
        }));
    }
    let unknown = PrivateKey::from(Ed25519Keypair::from_seed(&[63; 32]));
    assert!(matches!(
        certify(
            &source,
            *tag.as_handle().as_bytes(),
            &[unknown.public_key().to_openssh().unwrap()],
            &agent.public_keys,
            |_, _, _| panic!("no matching signing key")
        ),
        Err(Error::Untrusted)
    ));
}

#[test]
fn concurrent_signing_retains_each_keys_certificate() {
    let directory = Directory::new();
    let staging = directory.repository().unsigned();
    let fields = [
        Tree::empty().into(),
        Blob::literal(b"concurrent claim").unwrap().into(),
    ];
    let tag = Tree::canonical(&fields).unwrap().tag_descriptor();
    staging
        .publish(
            tag.into(),
            &fields
                .iter()
                .flat_map(|field| *field.as_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let keys: Vec<_> = [71, 72]
        .into_iter()
        .map(|seed| PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32])))
        .collect();
    let public_keys: Vec<_> = keys
        .iter()
        .map(|key| key.public_key().to_openssh().unwrap())
        .collect();
    let source = Repository::open(staging.path())
        .unwrap()
        .with_policy(Policy {
            authorized_keys: public_keys.clone(),
            ..Policy::default()
        });
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        for key in &keys {
            let source = &source;
            let barrier = &barrier;
            scope.spawn(move || {
                let identity = key.public_key().to_openssh().unwrap();
                certify(
                    source,
                    *tag.as_handle().as_bytes(),
                    core::slice::from_ref(&identity),
                    core::slice::from_ref(&identity),
                    |_, namespace, message| {
                        let signature = key
                            .sign(namespace, HashAlg::Sha512, message)?
                            .to_pem(ssh_key::LineEnding::LF)?;
                        barrier.wait();
                        Ok(signature)
                    },
                )
                .unwrap();
            });
        }
    });
    let certificates = source.tag_certificates(tag.into()).unwrap();
    assert_eq!(certificates.len(), 2);
    assert_eq!(
        source
            .certificate_blobs(tag.as_handle().as_bytes())
            .unwrap()
            .len(),
        2
    );
    for key in public_keys {
        assert!(certificates.iter().any(|certificate| {
            certificate
                .verify(
                    tag.as_handle().as_bytes(),
                    &Policy {
                        authorized_keys: vec![key.clone()],
                        ..Policy::default()
                    },
                )
                .is_ok()
        }));
    }
}

#[test]
fn requested_transfers_read_only_backing_missing_at_the_receiver() {
    let source = Directory::new();
    let source = source.repository();
    let target = Directory::new();
    let target = target.repository();
    let shared = source.create_blob(&[41; 128]).unwrap();
    target.create_blob(&[41; 128]).unwrap();
    let added = source.create_blob(&[42; 128]).unwrap();
    let root = source.create_tree(&[shared.into(), added.into()]).unwrap();
    let name = *root.as_handle().as_bytes();
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut connection = crate::linux::Connection(listener.accept().unwrap().0);
        assert_eq!(
            protocol::receive(&mut connection, 1 << 20).unwrap(),
            Some(protocol::Message::RequestTransfer(name))
        );
        let proposal = propose_export(&source, root.into(), true).unwrap();
        let mut read = Vec::new();
        transport::complete(protocol::asynchronous::send_proposal(
            &mut transport::Blocking(&mut connection),
            1 << 20,
            None,
            Some(name),
            proposal,
            |name| {
                assert_ne!(name, *shared.as_handle().as_bytes());
                read.push(name);
                let object = Object::try_from(source.lookup(&name).unwrap()).unwrap();
                source
                    .read_backing_range(object, 0, object.byte_len() as usize)
                    .map_err(|error| format!("{error}"))
            },
            |_, _| None,
        ))
        .unwrap();
        assert_eq!(read.len(), 2);
    });
    let mut connection = crate::linux::Connection::connect(address).unwrap();
    protocol::send(&mut connection, protocol::Message::RequestTransfer(name)).unwrap();
    let incoming = protocol::receive_transfer(&mut connection, 1 << 20, 1, &[], |offers| {
        Some(missing(&target, offers).unwrap())
    })
    .unwrap()
    .unwrap();
    assert!(incoming.task.is_none());
    assert_eq!(incoming.result, Some(name));
    import(&target, &incoming.transfer.encode().unwrap(), &config()).unwrap();
    assert!(target.read_tree(root).unwrap()[1].same_encoding(&added.into()));
    assert_eq!(target.read_blob(added).unwrap().as_ref(), &[42; 128]);
    server.join().unwrap();
}
