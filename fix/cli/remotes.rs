use crate::{Client, Shell, Value};
use fixremote::{
    linux::RemoteConnection,
    protocol::{self, Message, Transfer},
};
use fixrepository::{Remote, Repository};
use std::{error::Error, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn configured<'a>(remotes: &'a [Remote], name: &str) -> Result<&'a Remote> {
    remotes
        .iter()
        .find(|remote| remote.name == name)
        .ok_or_else(|| format!("unknown remote {name:?}").into())
}

fn split(reference: &str) -> Result<(&str, &str)> {
    let (remote, label) = reference.split_once(':').ok_or("expected REMOTE:LABEL")?;
    if remote.is_empty() || label.is_empty() {
        return Err("expected REMOTE:LABEL".into());
    }
    Ok((remote, label))
}

fn receive(stream: &mut RemoteConnection) -> Result<Message> {
    let waiting = common::progress::Progress::activity("Waiting for remote response");
    let mut transfer = None;
    let mut previous = 0;
    let result =
        protocol::receive_observed(stream, protocol::MAX_MESSAGE_SIZE, |received, total| {
            if received == 0 && total >= 1024 * 1024 {
                transfer = Some(common::progress::Progress::bytes(
                    "Receiving remote data",
                    total,
                ));
                waiting.message("receiving response");
            }
            if let Some(progress) = &transfer {
                progress.inc(received - previous);
            }
            previous = received;
            if received == total {
                if let Some(progress) = transfer.take() {
                    progress.finish();
                }
                waiting.message("decoding remote response");
            }
        })
        .map_err(|error| format!("remote protocol: {error}"))?
        .ok_or_else(|| {
            "remote closed without a response; check its scheduler and protocol version".into()
        });
    if result.is_ok() {
        waiting.finish();
    }
    result
}

fn send(stream: &mut RemoteConnection, message: Message) -> Result<()> {
    let encoding = common::progress::Progress::activity("Preparing remote request");
    let bytes = protocol::encode(message)?;
    encoding.finish();
    let progress = (bytes.len() >= 1024 * 1024)
        .then(|| common::progress::Progress::bytes("Sending remote data", bytes.len() as u64));
    let mut previous = 0;
    fixremote::transport::complete(fixremote::transport::asynchronous::send_observed(
        &mut fixremote::transport::Blocking(stream),
        &bytes,
        &mut |written, _| {
            if let Some(progress) = &progress {
                progress.inc(written - previous);
            }
            previous = written;
        },
    ))
    .map_err(|error| format!("remote transport: {error}"))?;
    if let Some(progress) = progress {
        progress.finish();
    }
    Ok(())
}

fn file_repository(remote: &Remote) -> Result<Option<Repository>> {
    remote
        .address
        .strip_prefix("file://")
        .map(|path| Repository::open(path).map_err(Into::into))
        .transpose()
}

pub(crate) fn file_refs(repository: &Repository) -> Result<Vec<(String, [u8; 32])>> {
    repository
        .labels()?
        .into_iter()
        .map(|label| {
            let name = *repository.read_label(&label)?.as_bytes();
            Ok((label, name))
        })
        .collect()
}

fn fetch_refs(remote: &Remote) -> Result<Vec<(String, [u8; 32])>> {
    let _progress =
        common::progress::Progress::activity(format!("Reading labels from {}", remote.name));
    if let Some(repository) = file_repository(remote)? {
        return file_refs(&repository);
    }
    let mut stream = RemoteConnection::connect(remote)?;
    send(&mut stream, Message::RequestLabels)?;
    match receive(&mut stream)? {
        Message::Labels(refs) => Ok(refs),
        Message::Failure(error) => Err(error.into()),
        reply => Err(format!("remote returned {reply:?} instead of labels").into()),
    }
}

pub fn clone(
    address: &str,
    directory: &Path,
    scheduler: Option<&str>,
    collector: Option<&str>,
) -> Result<()> {
    common::progress::println(format!("contacting {address} for remote refs"));
    let remote = Remote {
        name: "origin".into(),
        address: address.into(),
        scheduler: scheduler.map(str::to_owned),
        collector: collector.map(str::to_owned),
    };
    let refs = fetch_refs(&remote)?;
    match std::fs::create_dir(directory) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && directory.is_dir() => (),
        Err(error) => return Err(error.into()),
    }
    let path = directory.join(".fix");
    std::fs::create_dir(&path)?;
    let path = path.to_str().ok_or("repository path is not UTF-8")?;
    let repository = crate::commands::init_repository(path)?;
    let mut remotes = repository.remotes()?;
    remotes.push(Remote {
        scheduler: None,
        collector: None,
        ..remote
    });
    repository.set_remotes(remotes)?;
    let count = refs.len();
    repository.set_remote_refs("origin", refs)?;
    common::progress::println(format!(
        "cloned {} with {count} remote refs",
        directory.display()
    ));
    Ok(())
}

pub fn sync(shell: &Shell, remote: Option<&str>) -> Result<()> {
    let config = shell.remote_config()?;
    let remotes = match remote {
        Some(name) => std::slice::from_ref(configured(&config.remotes, name)?),
        None => config.remotes.as_slice(),
    };
    let repository = Repository::open(&shell.repository())?;
    let mut failures = Vec::new();
    for remote in remotes {
        common::progress::println(format!(
            "contacting {} for {} refs",
            remote.address, remote.name
        ));
        let result = fetch_refs(remote).and_then(|refs| {
            let count = refs.len();
            repository.set_remote_refs(&remote.name, refs)?;
            common::progress::println(format!(
                "synced {count} refs from {}; no objects fetched",
                remote.name
            ));
            Ok(())
        });
        if let Err(error) = result {
            failures.push(format!("remote {}: {error}", remote.name));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

pub(crate) fn resolve(repository: &Repository, reference: &str) -> Result<[u8; 32]> {
    let (remote, label) = split(reference)?;
    repository
        .remote_refs(remote)?
        .into_iter()
        .find(|(candidate, _)| candidate == label)
        .map(|(_, name)| name)
        .ok_or_else(|| format!("unknown remote ref {reference:?}; run fix sync {remote}").into())
}

pub(crate) fn fetch(client: &mut Client, reference: &str, backing: bool) -> Result<Value> {
    let progress = common::progress::Progress::activity(format!("Load {reference} -> local"));
    progress.message("locating the remote ref");
    let (remote, _) = split(reference)?;
    let config = client.remote_config()?;
    let configuration = configured(&config.remotes, remote)?;
    let repository = Repository::open(client.repository())?.with_policy(config.policy.clone());
    let file = file_repository(configuration)?;
    let name = resolve(&repository, reference)?;
    common::progress::println(format!(
        "fetching {reference} from {}",
        configuration.address
    ));
    progress.message("enumerating the remote value and its coupons");
    let (mut transfer, backing_names, mut stream) = if let Some(source) = &file {
        let root = source.lookup(&name)?;
        let (transfer, objects) = fixremote::propose_export(source, root, backing)
            .map_err(|error| format!("exporting {reference}: {error}"))?;
        (transfer, objects, None)
    } else {
        let mut stream = RemoteConnection::connect(configuration)?;
        send(
            &mut stream,
            Message::RequestObject {
                name,
                trusted_keys: config.policy.authorized_keys,
                backing,
            },
        )?;
        match receive(&mut stream)? {
            Message::ProposeObject { transfer, backing } => (transfer, backing, Some(stream)),
            Message::Failure(error) => return Err(format!("remote {remote}: {error}").into()),
            reply => {
                return Err(format!(
                    "remote {remote} returned {reply:?} while fetching {reference}"
                )
                .into());
            }
        }
    };
    if transfer.roots != [name] {
        return Err("remote returned a different value".into());
    }
    let mut records = std::collections::BTreeMap::new();
    for (index, record) in transfer.records.iter().enumerate() {
        if record.data.is_some() || records.insert(record.name, index).is_some() {
            return Err("remote supplied an invalid object manifest".into());
        }
    }
    let mut requested = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for name in backing_names {
        if !records.contains_key(&name) || !seen.insert(name) {
            return Err("remote offered backing outside its object manifest".into());
        }
        let handle = fixhandle::Handle::parse_ref(&name)
            .map_err(|error| format!("invalid object in remote manifest: {error:?}"))?;
        if !handle.is_canonical() || !matches!(handle.view(), fixhandle::HandleBytesView::Object(_))
        {
            return Err("remote offered backing for a noncanonical Object".into());
        }
        total = total
            .checked_add(handle.byte_len())
            .ok_or("remote transfer size overflow")?;
        requested.push(name);
    }
    if let Some(stream) = &mut stream {
        send(stream, Message::AcceptObject(requested.clone()))?;
    }
    progress.message(format!(
        "retrieving {} objects ({total} backing bytes)",
        requested.len()
    ));
    let download = common::progress::Progress::bytes(format!("Load {reference} -> local"), total);
    for name in requested {
        let length = fixhandle::Handle::parse_ref(&name).unwrap().byte_len();
        let mut data = Vec::new();
        let mut offset = 0;
        while offset < length {
            let chunk_length = (length - offset).min(protocol::OBJECT_CHUNK_SIZE as u64) as usize;
            let chunk =
                if let Some(source) = &file {
                    let object = fixhandle::Object::try_from(source.lookup(&name)?)
                        .map_err(|error| format!("invalid backing Object: {error:?}"))?;
                    let chunk = source
                        .read_backing_range(object, offset, chunk_length)
                        .map_err(|error| format!("reading {reference}: {error}"))?;
                    download.inc(chunk.len() as u64);
                    chunk
                } else {
                    let mut previous = 0;
                    let message = protocol::receive_observed(
                        stream.as_mut().unwrap(),
                        protocol::OBJECT_CHUNK_SIZE + 1024,
                        |received, total| {
                            let overhead = total.saturating_sub(chunk_length as u64);
                            let backing_received = received.saturating_sub(overhead);
                            download.inc(backing_received - previous);
                            previous = backing_received;
                        },
                    )
                    .map_err(|error| {
                        format!(
                            "receiving backing for {} at byte {offset}: {error}",
                            hex::encode(name)
                        )
                    })?;
                    match message {
                        Some(Message::ObjectChunk {
                            name: received,
                            offset: position,
                            data,
                        }) if received == name && position == offset => data,
                        Some(Message::Failure(error)) => {
                            return Err(format!("remote {remote}: {error}").into());
                        }
                        None => {
                            return Err(format!(
                                "remote closed before sending backing for {} at byte {offset}",
                                hex::encode(name)
                            )
                            .into());
                        }
                        _ => return Err(format!(
                            "remote sent an unexpected response for backing {} at byte {offset}",
                            hex::encode(name)
                        )
                        .into()),
                    }
                };
            if chunk.len() != chunk_length {
                return Err(format!(
                    "remote backing for {} at byte {offset} has {} bytes; expected {chunk_length}",
                    hex::encode(name),
                    chunk.len()
                )
                .into());
            }
            offset += chunk.len() as u64;
            data.extend_from_slice(&chunk);
        }
        transfer.records[records[&name]].data = Some(data);
    }
    download.finish();
    if !backing
        && matches!(
            fixhandle::Handle::parse_ref(&name)
                .map_err(|error| format!("invalid remote ref: {error:?}"))?
                .view(),
            fixhandle::HandleBytesView::Object(_)
        )
    {
        transfer.roots[0][31] |= 0x10;
    }
    progress.message("validating and storing the received value");
    let encoded = transfer.encode()?;
    let roots = client
        .import(&encoded)
        .map_err(|error| format!("importing {reference}: {error}"))?;
    let [value] = roots.as_slice() else {
        return Err("remote returned the wrong number of values".into());
    };
    progress.finish();
    Ok(value.clone())
}

pub(crate) fn expression(reference: &str) -> Result<crate::shell::Value> {
    Shell::with(|client| fetch(client, reference, false)).map(fix::Value::from_inner)
}

pub fn pull(
    _shell: &mut Shell,
    reference: &str,
    local: Option<&str>,
    shallow: bool,
    force: bool,
) -> Result<()> {
    let (_, label) = split(reference)?;
    Shell::with(|client| {
        let local = local.unwrap_or(label);
        if !force
            && Repository::open(client.repository())?
                .labels()?
                .iter()
                .any(|label| label == local)
        {
            return Err(format!(
                "local label {local:?} already exists; use --force to overwrite it"
            )
            .into());
        }
        let value = fetch(client, reference, !shallow)?;
        client.label_imported(local, &value)?;
        if shallow {
            common::progress::println(format!(
                "saved a shallow value for {reference} as local label {local}"
            ));
        } else {
            common::progress::println(format!("saved {reference} as local label {local}"));
        }
        Ok(())
    })
}

pub fn push(shell: &mut Shell, local: &str, reference: &str) -> Result<()> {
    let (remote, label) = split(reference)?;
    let config = shell.remote_config()?;
    let configuration = configured(&config.remotes, remote)?;
    let target = file_repository(configuration)?;
    if configuration.address.starts_with("tcp://") {
        return Err("push requires an SSH remote".into());
    }
    let repository = Repository::open(&shell.repository())?;
    let expected = repository
        .remote_refs(remote)?
        .into_iter()
        .find(|(candidate, _)| candidate == label)
        .map(|(_, name)| name);
    common::progress::println(format!(
        "pushing {local} to {reference} at {}",
        configuration.address
    ));
    let value = shell.label_value(local)?;
    let mut transfer = Transfer::decode(&shell.export(&[value], &[])?)?;
    let [name] = transfer.roots.as_slice() else {
        return Err("local label has no value".into());
    };
    let name = *name;
    if let Some(target) = target {
        let keys = target.policy()?.authorized_keys;
        transfer = protocol::prepare_transfer(transfer, &keys, |name, keys| {
            shell.certify(name, keys).map_err(|error| error.to_string())
        })?;
        let bytes = transfer.encode()?;
        fixremote::import(&target, &bytes, &config)?;
        let value = target.lookup(&name)?;
        target.compare_and_write_label(label, expected, value)?;
        let mut refs = repository.remote_refs(remote)?;
        if let Some((_, current)) = refs.iter_mut().find(|(candidate, _)| candidate == label) {
            *current = name;
        } else {
            refs.push((label.into(), name));
        }
        repository.set_remote_refs(remote, refs)?;
        common::progress::println(format!("updated remote label {reference}"));
        return Ok(());
    }
    let mut stream = RemoteConnection::connect(configuration)?;
    send(&mut stream, Message::RequestTrustedKeys)?;
    let keys = match receive(&mut stream)? {
        Message::TrustedKeys(keys) => keys,
        Message::Failure(error) => return Err(format!("remote {remote}: {error}").into()),
        reply => return Err(format!("remote returned {reply:?} instead of trusted keys").into()),
    };
    transfer = protocol::prepare_transfer(transfer, &keys, |name, keys| {
        shell.certify(name, keys).map_err(|error| error.to_string())
    })?;
    send(&mut stream, Message::SubmitObject(transfer))?;
    match receive(&mut stream)? {
        Message::ObjectReceived(received) if received == name => (),
        Message::Failure(error) => return Err(format!("remote {remote}: {error}").into()),
        reply => {
            return Err(
                format!("remote {remote} returned {reply:?} while importing the value").into(),
            );
        }
    }
    common::progress::println(format!("{remote} accepted the data for {reference}"));
    send(
        &mut stream,
        Message::UpdateLabel {
            label: label.into(),
            expected,
            value: name,
        },
    )?;
    match receive(&mut stream)? {
        Message::LabelUpdated => (),
        Message::Failure(error) => return Err(format!("remote {remote}: {error}").into()),
        reply => {
            return Err(
                format!("remote {remote} returned {reply:?} while updating {label}").into(),
            );
        }
    }
    let mut refs = repository.remote_refs(remote)?;
    if let Some((_, current)) = refs.iter_mut().find(|(candidate, _)| candidate == label) {
        *current = name;
    } else {
        refs.push((label.into(), name));
    }
    repository.set_remote_refs(remote, refs)?;
    common::progress::println(format!("updated remote label {reference}"));
    Ok(())
}
