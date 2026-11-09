//! Memory-only Git import over caller-provided byte streams.

use std::{collections::BTreeSet, io};

pub use gix_hash;
pub use gix_object;

mod pack;
mod transport;

pub use pack::Repository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    host: String,
    port: u16,
    repo_path: Vec<u8>,
    revision: Vec<u8>,
    selection: Vec<u8>,
}

impl Remote {
    pub fn parse(uri: &str) -> io::Result<Self> {
        let uri = uri
            .split_once("://")
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("git"))
            .map(|(_, uri)| uri)
            .ok_or_else(|| invalid("expected a git:// URI"))?;
        let (address, fragment) = uri.split_once('#').unwrap_or((uri, "HEAD"));
        let (authority, path) = address
            .split_once('/')
            .ok_or_else(|| invalid("missing repository path"))?;
        let (host, port) = if let Some(authority) = authority.strip_prefix('[') {
            let (host, suffix) = authority
                .split_once(']')
                .ok_or_else(|| invalid("invalid IPv6 authority"))?;
            let port = if suffix.is_empty() {
                9418
            } else {
                suffix
                    .strip_prefix(':')
                    .ok_or_else(|| invalid("invalid port"))?
                    .parse()
                    .map_err(invalid)?
            };
            (host, port)
        } else if let Some((host, port)) = authority.split_once(':') {
            (host, port.parse().map_err(invalid)?)
        } else {
            (authority, 9418)
        };
        if host.is_empty()
            || port == 0
            || host
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b == 0 || b == b'@')
        {
            return Err(invalid("invalid Git host or port"));
        }
        let repo_path = percent_decode(format!("/{path}").as_bytes())?;
        if repo_path
            .iter()
            .any(|b| *b == 0 || *b == b'\n' || *b == b'\r')
        {
            return Err(invalid("invalid Git repository path"));
        }
        let (revision, selection) = fragment.split_once(':').unwrap_or((fragment, ""));
        let revision = percent_decode(if revision.is_empty() {
            b"HEAD"
        } else {
            revision.as_bytes()
        })?;
        if revision.iter().any(|b| b.is_ascii_whitespace() || *b == 0) {
            return Err(invalid("invalid Git revision"));
        }
        let selection = percent_decode(selection.as_bytes())?;
        if !selection.is_empty()
            && selection
                .split(|b| *b == b'/')
                .any(|part| part.is_empty() || part == b"." || part == b".." || part.contains(&0))
        {
            return Err(invalid("selection must be a relative Git tree path"));
        }
        Ok(Self {
            host: host.into(),
            port,
            repo_path,
            revision,
            selection,
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn repo_path(&self) -> &[u8] {
        &self.repo_path
    }
    pub fn revision(&self) -> &[u8] {
        &self.revision
    }
    pub fn selection(&self) -> &[u8] {
        &self.selection
    }
}

#[derive(Debug, Clone)]
pub struct Object {
    pub kind: gix_object::Kind,
    pub data: Vec<u8>,
}

/// Fetches a self-contained shallow snapshot when supported by the server.
pub fn fetch(stream: impl io::Read + io::Write, remote: &Remote) -> io::Result<Repository> {
    transport::fetch(stream, remote, None)
}

/// Limits received pack bytes and individual decoding allocations.
pub fn fetch_with_limit(
    stream: impl io::Read + io::Write,
    remote: &Remote,
    limit: usize,
) -> io::Result<Repository> {
    transport::fetch(stream, remote, Some(limit))
}

impl Repository {
    pub fn resolve(&self, remote: &Remote) -> io::Result<Object> {
        let mut oid = transport::revision(&self.refs, remote.revision())?;
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(oid) {
                return Err(invalid("cyclic Git tag or commit"));
            }
            let object = self.object(oid.as_ref())?;
            oid = match object.kind {
                gix_object::Kind::Tag => {
                    gix_object::TagRefIter::from_bytes(&object.data, gix_hash::Kind::Sha1)
                        .target_id()
                        .map_err(invalid)?
                }
                gix_object::Kind::Commit => {
                    gix_object::CommitRefIter::from_bytes(&object.data, gix_hash::Kind::Sha1)
                        .tree_id()
                        .map_err(invalid)?
                }
                _ => break,
            };
        }
        for component in remote
            .selection()
            .split(|b| *b == b'/')
            .filter(|part| !part.is_empty())
        {
            let object = self.object(oid.as_ref())?;
            if object.kind != gix_object::Kind::Tree {
                return Err(not_found("selection traverses a non-tree object"));
            }
            let mut found = None;
            for entry in gix_object::TreeRefIter::from_bytes(&object.data, gix_hash::Kind::Sha1) {
                let entry = entry.map_err(invalid)?;
                if entry.filename == component {
                    if entry.mode.is_commit() {
                        return Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            "Git submodules are not imported",
                        ));
                    }
                    if !(entry.mode.is_tree() || entry.mode.is_blob() || entry.mode.is_link()) {
                        return Err(invalid("unsupported Git tree entry mode"));
                    }
                    found = Some((entry.oid.to_owned(), entry.mode.is_tree()));
                    break;
                }
            }
            let (selected, is_tree) = found.ok_or_else(|| not_found("Git tree path not found"))?;
            let object = self.object(selected.as_ref())?;
            let expected = if is_tree {
                gix_object::Kind::Tree
            } else {
                gix_object::Kind::Blob
            };
            if object.kind != expected {
                return Err(invalid("Git tree entry has the wrong object kind"));
            }
            oid = selected;
        }
        self.object(oid.as_ref())
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn not_found(error: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, error)
}

fn percent_decode(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoded = Vec::new();
    let mut input = bytes.iter().copied();
    while let Some(byte) = input.next() {
        decoded.push(if byte == b'%' {
            let hi = input.next().and_then(|b| (b as char).to_digit(16));
            let lo = input.next().and_then(|b| (b as char).to_digit(16));
            ((hi.ok_or_else(|| invalid("invalid URI escape"))? << 4)
                | lo.ok_or_else(|| invalid("invalid URI escape"))?) as u8
        } else {
            byte
        });
    }
    Ok(decoded)
}
