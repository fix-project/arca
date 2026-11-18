use crate::Shell;
use crate::shell::{Object, Reference, Thunk, Tree, Value};
use fix::ValueType;
use fixremote::{
    linux::RemoteConnection,
    protocol::{self, Message},
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
            let name = repository.label_name(&label)?;
            Ok((label, name))
        })
        .collect()
}

fn fetch_refs(remote: &Remote, repository: Option<&Repository>) -> Result<Vec<(String, [u8; 32])>> {
    let _progress =
        common::progress::Progress::activity(format!("Reading labels from {}", remote.name));
    let result = (|| -> Result<_> {
        if let Some(repository) = file_repository(remote)? {
            return file_refs(&repository);
        }
        let mut stream = RemoteConnection::connect(remote)?;
        send(&mut stream, Message::RequestLabels)?;
        receive_labels(&mut stream, repository)
    })();
    result.map_err(|error| {
        format!(
            "reading labels from {} ({}): {error}",
            remote.name, remote.address
        )
        .into()
    })
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
    let refs = fetch_refs(&remote, None)?;
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
    let snapshot = protocol::encode_labels(&refs)?;
    repository.set_remote_refs("origin", refs)?;
    repository.create_blob(&snapshot)?;
    common::progress::println(format!(
        "cloned {} with {count} remote refs",
        directory.display()
    ));
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Shallow,
    Strict,
    Exact,
}

impl std::str::FromStr for Mode {
    type Err = Box<dyn Error>;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "shallow" => Ok(Self::Shallow),
            "strict" => Ok(Self::Strict),
            "exact" => Ok(Self::Exact),
            _ => Err(format!(
                "unknown transfer mode {value:?}; expected shallow, strict, or exact"
            )
            .into()),
        }
    }
}

impl Mode {
    pub fn configured(operation: &str, override_mode: Option<Self>) -> Result<Self> {
        if let Some(mode) = override_mode {
            return Ok(mode);
        }
        crate::configuration::settings()?
            .get(&format!("{operation}.mode"))
            .map(|value| value.parse())
            .unwrap_or(Ok(Self::default()))
    }

    fn realize(self, value: Value) -> Result<Value> {
        let value = self.encode(value)?;
        if self == Self::Exact {
            Shell::with(|client| client.load(value.inner()))
                .map(Value::from_inner)
                .map_err(Into::into)
        } else {
            value.eval().map_err(Into::into)
        }
    }

    fn realize_all(self, values: Vec<Value>) -> Result<Vec<Value>> {
        let values = values
            .into_iter()
            .map(|value| self.encode(value))
            .collect::<Result<Vec<_>>>()?;
        let tree = Tree::new(&values).into_value();
        let result = if self == Self::Exact {
            Shell::with(|client| client.load(tree.inner())).map(Value::from_inner)?
        } else {
            tree.eval()?
        };
        Tree::try_from(result)?
            .into_iter()
            .map(|value| value.map_err(Into::into))
            .collect()
    }

    fn encode(self, value: Value) -> Result<Value> {
        if self == Self::Exact {
            return Ok(value);
        }
        if self == Self::Shallow {
            match value.value_type() {
                ValueType::Object => return Ok(Object::try_from(value)?.reference().into_value()),
                ValueType::Ref => return Ok(value),
                _ => (),
            }
        }
        let thunk = match value.value_type() {
            ValueType::Object => Object::try_from(value)?.reference().identification(),
            ValueType::Ref => Reference::try_from(value)?.identification(),
            ValueType::Thunk => Thunk::try_from(value)?,
            ValueType::Encode => {
                let value = Shell::with(|client| -> std::result::Result<_, crate::Error> {
                    let handle = client.handle(value.inner())?;
                    let fixhandle::HandleView::Encode(encode) = handle.view() else {
                        unreachable!()
                    };
                    let thunk = match encode.view() {
                        fixhandle::EncodeView::Strict(thunk)
                        | fixhandle::EncodeView::Shallow(thunk) => thunk,
                    };
                    Ok(Value::from_inner(client.keep(thunk.into())))
                })?;
                Thunk::try_from(value)?
            }
        };
        let encoded = match self {
            Self::Shallow => thunk.shallow(),
            Self::Strict => thunk.strict(),
            Self::Exact => unreachable!(),
        };
        Ok(encoded.into_value())
    }
}

pub(crate) fn resolve(repository: &Repository, reference: &str) -> Result<[u8; 32]> {
    Ok(*repository.read_label(reference)?.as_bytes())
}

pub fn pull(
    shell: &mut Shell,
    remote: Option<&str>,
    source: Option<&str>,
    destination: Option<&str>,
    mode: Mode,
) -> Result<()> {
    let config = shell.remote_config()?;
    let remotes = match remote {
        Some(name) => std::slice::from_ref(configured(&config.remotes, name)?),
        None => config.remotes.as_slice(),
    };
    if source.is_some() && remote.is_none() {
        return Err("pull SOURCE requires a remote".into());
    }
    let repository = Repository::open(&shell.repository())?;
    let mut failures = Vec::new();
    let mut selected = Vec::new();
    for remote in remotes {
        match pull_remote(&repository, remote, source, destination) {
            Ok(labels) => selected.extend(labels),
            Err(error) => failures.push(format!("remote {}: {error}", remote.name)),
        }
    }
    if !selected.is_empty() {
        let progress =
            common::progress::Progress::advised(format!("Pull {} labels", selected.len()));
        let values = selected
            .iter()
            .map(|(reference, _, _)| shell.label_value(reference))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let results = mode
            .realize_all(values)
            .map_err(|error| format!("pulling {} labels: {error}", selected.len()))?;
        if results.len() != selected.len() {
            return Err(format!(
                "pulling {} labels: scheduler returned {} results",
                selected.len(),
                results.len()
            )
            .into());
        }
        progress.message("Saving labels");
        for ((_, target, name), mut value) in selected.into_iter().zip(results) {
            if mode == Mode::Exact {
                value = shell.lookup(&hex::encode(name))?;
            }
            Shell::with(|client| client.label_imported(&target, value.inner()))?;
        }
        progress.finish();
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn pull_remote(
    repository: &Repository,
    remote: &Remote,
    source: Option<&str>,
    destination: Option<&str>,
) -> Result<Vec<(String, String, [u8; 32])>> {
    let refs = fetch_refs(remote, Some(repository))?;
    let selected = match source {
        Some(source) => vec![
            refs.iter()
                .find(|(label, _)| label == source)
                .cloned()
                .ok_or_else(|| format!("remote {} has no label {source:?}", remote.name))?,
        ],
        None => refs.clone(),
    };
    let refs = if source.is_some() {
        let mut current = repository.remote_refs(&remote.name)?;
        current.retain(|(label, _)| selected.iter().all(|(source, _)| source != label));
        current.extend(selected.clone());
        current
    } else {
        refs
    };
    repository.set_remote_refs(&remote.name, refs)?;
    Ok(selected
        .into_iter()
        .map(|(label, name)| {
            let reference = format!("{}:{label}", remote.name);
            let target = destination.unwrap_or(&reference).to_owned();
            (reference, target, name)
        })
        .collect())
}

pub fn push(
    shell: &mut Shell,
    remote: &str,
    source: Option<&str>,
    destination: Option<&str>,
    mode: Mode,
) -> Result<()> {
    let config = shell.remote_config()?;
    let configuration = configured(&config.remotes, remote)?;
    if configuration.address.starts_with("tcp://") {
        return Err("push requires an SSH or file remote".into());
    }
    let repository = Repository::open(&shell.repository())?;
    let labels = match source {
        Some(source) => vec![source.to_owned()],
        None => repository.labels()?,
    };
    let target = file_repository(configuration)?;
    let mut stream = if target.is_none() {
        Some(RemoteConnection::connect(configuration)?)
    } else {
        None
    };
    let current = if let Some(target) = &target {
        file_refs(target)?
    } else {
        let stream = stream.as_mut().unwrap();
        send(stream, Message::RequestLabels)?;
        receive_labels(stream, Some(&repository))?
    };
    for local in labels {
        let label = destination.unwrap_or(&local);
        let reference = format!("{remote}:{label}");
        let value = shell.label_value(&local)?;
        let value = mode
            .realize(value)
            .map_err(|error| format!("preparing {local} for {reference}: {error}"))?;
        let value = if mode == Mode::Exact {
            value
        } else {
            Mode::Exact
                .realize(value)
                .map_err(|error| format!("loading backing for {reference}: {error}"))?
        };
        let name = shell.save(&value)?;
        let mut bytes = [0; 32];
        hex::decode_to_slice(&name, &mut bytes)
            .map_err(|error| format!("invalid saved Fix name {name}: {error}"))?;
        let name = bytes;
        let root = repository.lookup(&name)?;
        let (mut transfer, backing) = fixremote::propose_export_full(&repository, root)?;
        let expected = current
            .iter()
            .find(|(candidate, _)| candidate == label)
            .map(|(_, name)| *name);
        if let Some(target) = &target {
            let offers: Vec<_> = transfer
                .records
                .iter()
                .map(|record| {
                    let mut offer = record.offer();
                    offer.backing = backing.contains(&record.name);
                    offer
                })
                .collect();
            let missing = fixremote::missing(target, &offers)?;
            for record in &mut transfer.records {
                if missing.contains(&record.name) && backing.contains(&record.name) {
                    record.data = Some(read_backing(&repository, record.name)?);
                } else {
                    record.certificates.clear();
                }
            }
            let keys = target.policy()?.authorized_keys;
            transfer = protocol::prepare_transfer(transfer, &keys, |name, keys| {
                shell.certify(name, keys).map_err(|error| error.to_string())
            })?;
            fixremote::import(target, &transfer.encode()?, &config)?;
            let value = target.lookup(&name)?;
            target.compare_and_write_label(label, expected, value)?;
        } else {
            let stream = stream.as_mut().unwrap();
            fixremote::transport::complete(protocol::asynchronous::send_proposal(
                &mut fixremote::transport::Blocking(&mut *stream),
                protocol::MAX_MESSAGE_SIZE,
                None,
                Some(name),
                (transfer, backing),
                |name| read_backing(&repository, name).map_err(|error| error.to_string()),
                |name, keys| shell.certify(name, keys).ok(),
            ))
            .map_err(|error| format!("transferring {local} to {reference}: {error}"))?;
            match receive(stream)? {
                Message::ObjectReceived(received) if received == name => (),
                Message::Failure(error) => return Err(format!("remote {remote}: {error}").into()),
                reply => {
                    return Err(format!(
                        "remote {remote} returned {reply:?} while importing {local}"
                    )
                    .into());
                }
            }
            send(
                stream,
                Message::UpdateLabel {
                    label: label.into(),
                    expected,
                    value: name,
                },
            )?;
            match receive(stream)? {
                Message::LabelUpdated => (),
                Message::Failure(error) => {
                    return Err(format!("updating {reference}: {error}").into());
                }
                reply => {
                    return Err(format!(
                        "remote {remote} returned {reply:?} while updating {reference}"
                    )
                    .into());
                }
            }
        }
        let mut refs = repository.remote_refs(remote)?;
        refs.retain(|(candidate, _)| candidate != label);
        refs.push((label.into(), name));
        repository.set_remote_refs(remote, refs)?;
        common::progress::println(format!("updated {reference} from {local}"));
    }
    Ok(())
}

fn read_backing(repository: &Repository, name: [u8; 32]) -> Result<Vec<u8>> {
    let object = fixhandle::Object::try_from(repository.lookup(&name)?)
        .map_err(fixrepository::RepositoryError::from)?;
    let length = usize::try_from(object.byte_len())?;
    Ok(repository.read_backing_range(object, 0, length)?)
}

fn receive_labels(
    stream: &mut RemoteConnection,
    repository: Option<&Repository>,
) -> Result<Vec<(String, [u8; 32])>> {
    let incoming =
        protocol::receive_transfer(stream, protocol::MAX_MESSAGE_SIZE, 0, &[], |offers| {
            match repository {
                Some(repository) => fixremote::missing(repository, offers).ok(),
                None => Some(offers.iter().map(|offer| offer.name).collect()),
            }
        })
        .map_err(|error| format!("receiving label snapshot: {error}"))?
        .ok_or("remote closed without providing its label snapshot")?;
    let [name] = incoming.transfer.roots.as_slice() else {
        return Err("remote label snapshot must contain one Blob".into());
    };
    if incoming.task.is_some() || incoming.result != Some(*name) {
        return Err("unexpected evaluation result while reading remote labels".into());
    }
    let handle = fixhandle::Handle::parse_ref(name)
        .map_err(|error| format!("invalid label snapshot name: {error:?}"))?;
    if !handle.is_canonical() {
        return Err("remote label snapshot is not canonical".into());
    }
    let blob = fixhandle::Blob::try_from(unsafe { handle.to_owned() })
        .map_err(fixrepository::RepositoryError::from)?;
    if incoming
        .transfer
        .records
        .iter()
        .any(|record| record.name != *name)
        || !incoming.transfer.certificates.is_empty()
    {
        return Err("remote label snapshot contains unrelated objects".into());
    }
    let bytes = if let Some(bytes) = blob.literal_bytes() {
        bytes.to_vec()
    } else if let Some(data) = incoming
        .transfer
        .records
        .iter()
        .find_map(|record| record.data.as_ref())
    {
        data.clone()
    } else {
        repository
            .ok_or("remote did not send its label snapshot")?
            .read_blob(blob)?
            .into_vec()
    };
    blob.verify(&bytes)
        .map_err(fixrepository::RepositoryError::from)?;
    let labels = protocol::decode_labels(&bytes)?;
    if let Some(repository) = repository {
        repository.create_blob(&bytes)?;
    }
    Ok(labels)
}
