use std::io::{self, Cursor, Read, Write};

use fixgit::{Remote, Repository, fetch, fetch_with_limit, gix_hash, gix_object};

const PACK: &[u8] = include_bytes!("fixture.pack");
const REFS: &str = include_str!("fixture.refs");

struct Peer {
    responses: Cursor<Vec<u8>>,
}

impl Read for Peer {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let length = out.len().min(7);
        self.responses.read(&mut out[..length])
    }
}
impl Write for Peer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len().min(11))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn packet(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(format!("{:04x}", bytes.len() + 4).as_bytes());
    output.extend_from_slice(bytes);
}

fn peer(pack: &[u8], sidebands: bool, shallow: bool) -> Peer {
    let mut responses = Vec::new();
    let capabilities = format!(
        "ofs-delta no-progress{}{}",
        if sidebands { " side-band-64k" } else { "" },
        if shallow { " shallow" } else { "" }
    );
    for (index, line) in REFS
        .lines()
        .filter(|line| !line.starts_with("blob "))
        .enumerate()
    {
        let (name, oid) = line.split_once(' ').unwrap();
        packet(
            &mut responses,
            format!(
                "{oid} {name}{}\n",
                if index == 0 {
                    format!("\0{capabilities}")
                } else {
                    String::new()
                }
            )
            .as_bytes(),
        );
    }
    responses.extend_from_slice(b"0000");
    if shallow {
        let head = REFS.lines().next().unwrap().split_once(' ').unwrap().1;
        packet(&mut responses, format!("shallow {head}\n").as_bytes());
        responses.extend_from_slice(b"0000");
    }
    packet(&mut responses, b"NAK\n");
    if sidebands {
        packet(&mut responses, b"\x02progress\n");
        for bytes in pack.chunks(137) {
            let mut band = vec![1];
            band.extend_from_slice(bytes);
            packet(&mut responses, &band);
        }
        responses.extend_from_slice(b"0000");
    } else {
        responses.extend_from_slice(pack);
    }
    Peer {
        responses: Cursor::new(responses),
    }
}

fn repository() -> Repository {
    fetch(
        peer(PACK, true, false),
        &Remote::parse("git://localhost/repo").unwrap(),
    )
    .unwrap()
}

#[test]
fn custom_transport_handles_partial_reads_and_writes() {
    let remote = Remote::parse("git://localhost:1234/repo#main:nested/child").unwrap();
    let mut peer = peer(PACK, true, true);
    let repository = fetch(&mut peer, &remote).unwrap();
    assert_eq!(repository.resolve(&remote).unwrap().data, b"hello world\n");
}

#[test]
fn annotated_tags_trees_blob_ids_and_file_modes_are_preserved() {
    let repository = repository();
    let tagged = Remote::parse("git://localhost/repo#v1:file").unwrap();
    assert_eq!(repository.resolve(&tagged).unwrap().data, b"hello world!\n");
    let root = repository
        .resolve(&Remote::parse("git://localhost/repo#refs/heads/main").unwrap())
        .unwrap();
    assert_eq!(root.kind, gix_object::Kind::Tree);
    let entries: Vec<_> = gix_object::TreeRefIter::from_bytes(&root.data, gix_hash::Kind::Sha1)
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(entries[0].mode.is_executable());
    assert!(entries[1].mode.is_link());
    assert_eq!(entries[2].filename, b"nested.c".as_slice());
    assert_eq!(entries[3].filename, b"nested".as_slice());
    assert!(entries[3].mode.is_tree());
    assert!(entries[5].mode.is_commit());
    assert_eq!(
        repository
            .resolve(&Remote::parse("git://localhost/repo#HEAD:payload").unwrap())
            .unwrap()
            .data,
        (0u8..96).collect::<Vec<_>>()
    );
    let oid = REFS
        .lines()
        .find_map(|line| line.strip_prefix("blob "))
        .unwrap();
    assert_eq!(
        repository
            .resolve(&Remote::parse(&format!("git://localhost/repo#{oid}")).unwrap())
            .unwrap()
            .data,
        b"hello world!\n"
    );
    assert_eq!(
        repository
            .resolve(&Remote::parse("git://localhost/repo#HEAD:submodule").unwrap())
            .unwrap_err()
            .kind(),
        io::ErrorKind::Unsupported
    );
}

#[test]
fn raw_pack_and_forward_reference_deltas_are_supported() {
    let remote = Remote::parse("git://localhost/repo").unwrap();
    let repository = fetch(peer(PACK, false, false), &remote).unwrap();
    let expected = gix_object::compute_hash(
        gix_hash::Kind::Sha1,
        gix_object::Kind::Blob,
        b"hello changed\n",
    )
    .unwrap();
    assert_eq!(
        repository.object(expected.as_ref()).unwrap().data,
        b"hello changed\n"
    );
}

#[test]
fn invalid_truncated_and_oversized_pack_inputs_fail() {
    let remote = Remote::parse("git://localhost/repo").unwrap();
    for length in [0, 4, 12, PACK.len() - 1] {
        assert!(fetch(peer(&PACK[..length], true, false), &remote).is_err());
    }
    let mut corrupt = PACK.to_vec();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(fetch(peer(&corrupt, true, false), &remote).is_err());
    assert!(fetch_with_limit(peer(PACK, true, false), &remote, PACK.len() - 1).is_err());
    assert!(
        fetch(
            peer(PACK, true, false),
            &Remote::parse("git://localhost/repo#missing").unwrap()
        )
        .is_err()
    );
}

fn checksum(pack: &mut [u8]) {
    let end = pack.len() - 20;
    let mut hash = gix_hash::hasher(gix_hash::Kind::Sha1);
    hash.update(&pack[..end]);
    pack[end..].copy_from_slice(hash.try_finalize().unwrap().as_bytes());
}

#[test]
fn self_consistent_checksums_do_not_hide_missing_bases_or_wrong_counts() {
    let remote = Remote::parse("git://localhost/repo").unwrap();
    let mut missing_base = PACK.to_vec();
    missing_base[13] ^= 1;
    checksum(&mut missing_base);
    assert!(fetch(peer(&missing_base, true, false), &remote).is_err());
    let mut wrong_count = PACK.to_vec();
    wrong_count[11] -= 1;
    checksum(&mut wrong_count);
    assert!(fetch(peer(&wrong_count, true, false), &remote).is_err());
}

#[test]
fn remote_errors_and_truncated_sidebands_fail() {
    let remote = Remote::parse("git://localhost/repo").unwrap();
    let mut peer = peer(PACK, true, false);
    peer.responses.get_mut().truncate(12);
    assert!(fetch(peer, &remote).is_err());
    let mut responses = Vec::new();
    packet(&mut responses, b"ERR repository unavailable\n");
    let error = fetch(
        Peer {
            responses: Cursor::new(responses),
        },
        &remote,
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("repository unavailable"));
}

#[test]
fn server_pack_errors_are_reported() {
    let remote = Remote::parse("git://localhost/repo").unwrap();
    let mut peer = peer(PACK, true, false);
    let responses = peer.responses.get_mut();
    let ack = responses
        .windows(8)
        .position(|bytes| bytes == b"0008NAK\n")
        .unwrap()
        + 8;
    responses.truncate(ack);
    packet(responses, b"\x03access denied\n");
    responses.extend_from_slice(b"0000");
    assert!(
        fetch(peer, &remote)
            .err()
            .unwrap()
            .to_string()
            .contains("access denied")
    );
}

#[test]
fn uri_paths_are_bytes_and_connection_parameters_are_validated() {
    let remote = Remote::parse("GIT://[::1]:1234/repo%20name#refs/tags/v1:nested/%ff").unwrap();
    assert_eq!(remote.host(), "::1");
    assert_eq!(remote.port(), 1234);
    assert_eq!(remote.repo_path(), b"/repo name");
    assert_eq!(remote.selection(), b"nested/\xff");
    for uri in [
        "https://host/repo",
        "git://host",
        "git://host:0/repo",
        "git://host/repo%00",
        "git://host/repo#HEAD:../file",
        "git://host/repo#HEAD:%zz",
    ] {
        assert!(Remote::parse(uri).is_err(), "{uri}");
    }
}

#[test]
fn versioned_and_shallow_server_advertisements_are_supported() {
    let remote = Remote::parse("git://localhost/repo").unwrap();
    let mut peer = peer(PACK, true, true);
    let responses = peer.responses.get_mut();
    let flush = responses
        .windows(4)
        .position(|bytes| bytes == b"0000")
        .unwrap();
    let head = REFS.lines().next().unwrap().split_once(' ').unwrap().1;
    let mut shallow = Vec::new();
    packet(&mut shallow, format!("shallow {head}\n").as_bytes());
    responses.splice(flush..flush, shallow);
    let mut version = Vec::new();
    packet(&mut version, b"version 1\n");
    responses.splice(0..0, version);
    assert_eq!(
        fetch(peer, &remote).unwrap().resolve(&remote).unwrap().kind,
        gix_object::Kind::Tree
    );
}
