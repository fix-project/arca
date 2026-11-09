use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
};

use gix_hash::ObjectId;
use gix_packetline::{
    PacketLineRef,
    blocking_io::{StreamingPeekableIter, encode},
};

use crate::{Remote, Repository, invalid, not_found, pack::allocation_limit};

type Refs = BTreeMap<Vec<u8>, ObjectId>;

pub(crate) fn revision(refs: &Refs, revision: &[u8]) -> io::Result<ObjectId> {
    if revision.len() == 40 && revision.iter().all(u8::is_ascii_hexdigit) {
        return ObjectId::from_hex(revision).map_err(invalid);
    }
    if let Some(oid) = refs.get(revision) {
        return Ok(*oid);
    }
    let mut selected = None;
    for prefix in [b"refs/heads/".as_slice(), b"refs/tags/"] {
        let name: Vec<_> = prefix.iter().chain(revision).copied().collect();
        if let Some(oid) = refs.get(&name) {
            if selected.is_some_and(|selected| selected != *oid) {
                return Err(invalid(
                    "ambiguous Git branch or tag; use its full ref name",
                ));
            }
            selected = Some(*oid);
        }
    }
    selected.ok_or_else(|| not_found("Git revision was not advertised"))
}

pub(crate) fn fetch(
    mut stream: impl Read + Write,
    remote: &Remote,
    limit: Option<usize>,
) -> io::Result<Repository> {
    let mut service = b"git-upload-pack ".to_vec();
    service.extend_from_slice(remote.repo_path());
    service.push(0);
    service.extend_from_slice(
        format!(
            "host={}{}\0",
            remote.host(),
            if remote.port() == 9418 {
                String::new()
            } else {
                format!(":{}", remote.port())
            }
        )
        .as_bytes(),
    );
    encode::write_packet_line(&PacketLineRef::Data(&service), &mut stream)?;
    stream.flush()?;
    let mut reader = StreamingPeekableIter::new(&mut stream, &[PacketLineRef::Flush], false);
    reader.fail_on_err_lines(true);
    let mut refs = Refs::new();
    let mut capabilities = Vec::new();
    let mut first = true;
    while let Some(line) = reader.read_line() {
        let bytes = line?
            .map_err(invalid)?
            .as_slice()
            .ok_or_else(|| invalid("expected a Git ref advertisement"))?;
        let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        if first && bytes == b"version 1" {
            continue;
        }
        if let Some(oid) = bytes.strip_prefix(b"shallow ") {
            ObjectId::from_hex(oid).map_err(invalid)?;
            continue;
        }
        let (record, caps) = split_once(bytes, 0).unwrap_or((bytes, &[]));
        if first {
            capabilities.extend_from_slice(caps);
        } else if !caps.is_empty() {
            return Err(invalid("capabilities appeared after the first Git ref"));
        }
        first = false;
        let (oid, name) =
            split_once(record, b' ').ok_or_else(|| invalid("invalid Git ref advertisement"))?;
        let oid = ObjectId::from_hex(oid).map_err(invalid)?;
        if !oid.is_null() && refs.insert(name.to_vec(), oid).is_some() {
            return Err(invalid("duplicate Git ref advertisement"));
        }
    }
    if reader.stopped_at() != Some(PacketLineRef::Flush) {
        return Err(invalid("missing Git ref advertisement flush"));
    }
    let wanted = revision(&refs, remote.revision())?;
    let capabilities: Vec<_> = capabilities.split(|b| b.is_ascii_whitespace()).collect();
    let has = |name: &[u8]| capabilities.contains(&name);
    let sideband = has(b"side-band-64k") || has(b"side-band");
    let shallow = has(b"shallow");
    let mut want = format!("want {wanted}");
    for capability in [b"ofs-delta".as_slice(), b"no-progress"] {
        if has(capability) {
            want.push(' ');
            want.push_str(std::str::from_utf8(capability).unwrap());
        }
    }
    if sideband {
        want.push_str(if has(b"side-band-64k") {
            " side-band-64k"
        } else {
            " side-band"
        });
    }
    want.push('\n');
    let mut stream = reader.into_inner();
    encode::write_packet_line(&PacketLineRef::Data(want.as_bytes()), &mut stream)?;
    if shallow {
        encode::write_packet_line(&PacketLineRef::Data(b"deepen 1\n"), &mut stream)?;
    }
    encode::flush_to_write(&mut stream)?;
    encode::write_packet_line(&PacketLineRef::Data(b"done\n"), &mut stream)?;
    stream.flush()?;
    let mut reader = StreamingPeekableIter::new(stream, &[PacketLineRef::Flush], false);
    reader.fail_on_err_lines(true);
    if shallow {
        while let Some(line) = reader.read_line() {
            let bytes = line?
                .map_err(invalid)?
                .as_slice()
                .ok_or_else(|| invalid("invalid Git shallow response"))?;
            let oid = bytes
                .strip_prefix(b"shallow ")
                .or_else(|| bytes.strip_prefix(b"unshallow "))
                .ok_or_else(|| invalid("invalid Git shallow response"))?;
            ObjectId::from_hex(oid.strip_suffix(b"\n").unwrap_or(oid)).map_err(invalid)?;
        }
        if reader.stopped_at() != Some(PacketLineRef::Flush) {
            return Err(invalid("missing Git shallow-response flush"));
        }
        reader.reset();
    }
    let ack = reader
        .read_line()
        .ok_or_else(|| invalid("missing Git acknowledgement"))??
        .map_err(invalid)?;
    if ack.as_slice() != Some(b"NAK\n") {
        return Err(invalid("unexpected Git acknowledgement without any haves"));
    }
    let mut pack = Vec::new();
    if sideband {
        while let Some(line) = reader.read_line() {
            let bytes = line?
                .map_err(invalid)?
                .as_slice()
                .ok_or_else(|| invalid("invalid Git pack sideband"))?;
            match bytes.split_first() {
                Some((1, bytes)) => append(&mut pack, bytes, limit)?,
                Some((2, _)) => (),
                Some((3, bytes)) => {
                    return Err(invalid(format!(
                        "Git server: {}",
                        String::from_utf8_lossy(bytes)
                    )));
                }
                _ => return Err(invalid("invalid Git pack sideband channel")),
            }
        }
        if reader.stopped_at() != Some(PacketLineRef::Flush) {
            return Err(invalid("missing Git pack flush"));
        }
    } else {
        let stream = reader.into_inner();
        let mut chunk = [0; 65536];
        loop {
            let length = stream.read(&mut chunk)?;
            if length == 0 {
                break;
            }
            append(&mut pack, &chunk[..length], limit)?;
        }
    }
    Repository::from_pack(pack, refs, limit)
}

fn append(output: &mut Vec<u8>, bytes: &[u8], limit: Option<usize>) -> io::Result<()> {
    if limit.is_some_and(|limit| bytes.len() > limit.saturating_sub(output.len())) {
        return Err(allocation_limit());
    }
    output.try_reserve(bytes.len()).map_err(invalid)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn split_once(bytes: &[u8], delimiter: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == delimiter)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}
