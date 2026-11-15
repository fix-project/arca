use crate::{Error, Policy, public_key};
use alloc::{format, string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
};

pub fn directory() -> Result<PathBuf, Error> {
    env::var_os("FIX_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".fix/config")))
        .ok_or(Error::Invalid)
}

fn read(name: &str) -> Result<String, Error> {
    match fs::read_to_string(directory()?.join(name)) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(Error::Configuration(format!(
            "cannot read user configuration: {error}"
        ))),
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default)]
    identities: Vec<String>,
}

pub fn load() -> Result<Policy, Error> {
    if directory().is_err() {
        return Ok(Policy {
            unsigned: env::var_os("FIX_UNSIGNED")
                .is_some_and(|value| value == "true" || value == "1"),
            ..Policy::default()
        });
    }
    let config = read("config")?;
    let config: Config = if config.trim().is_empty() {
        Config::default()
    } else {
        serde_json::from_str(&config).map_err(|_| Error::Invalid)?
    };
    let authorized_keys = read("authorized_keys")?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect::<Vec<_>>();
    for key in authorized_keys.iter().chain(&config.identities) {
        public_key(key)?;
    }
    let trusted_compilers = read("trusted_compilers")?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect::<Vec<_>>();
    let trusted_taggers = read("trusted_taggers")?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect::<Vec<_>>();
    for name in trusted_compilers.iter().chain(&trusted_taggers) {
        compiler(name)?;
    }
    Ok(Policy {
        identities: config.identities,
        authorized_keys,
        trusted_compilers,
        trusted_taggers,
        unsigned: env::var_os("FIX_UNSIGNED").is_some_and(|value| value == "1" || value == "true"),
    })
}

pub fn compiler(text: &str) -> Result<(), Error> {
    let mut bytes = [0; 32];
    hex::decode_to_slice(text, &mut bytes).map_err(|_| Error::Invalid)?;
    if !fixhandle::Handle::parse_ref(&bytes)
        .map_err(|_| Error::Invalid)?
        .is_canonical()
    {
        return Err(Error::Invalid);
    }
    Ok(())
}

pub fn key(text: &str) -> Result<String, Error> {
    let text = if text.starts_with("ssh-") {
        text.into()
    } else {
        fs::read_to_string(text)
            .map_err(|error| Error::Configuration(format!("cannot read public key: {error}")))?
    };
    let key = public_key(text.trim())?;
    ssh_key::PublicKey::new(key.key_data().clone(), "")
        .to_openssh()
        .map_err(Error::from)
}

pub fn update(kind: &str, value: &str, add: bool) -> Result<(), Error> {
    let mut policy = load()?;
    let (entries, value) = match kind {
        "key" => (&mut policy.authorized_keys, key(value)?),
        "identity" => (&mut policy.identities, key(value)?),
        "compiler" | "tagger" => {
            compiler(value)?;
            (
                if kind == "compiler" {
                    &mut policy.trusted_compilers
                } else {
                    &mut policy.trusted_taggers
                },
                value.into(),
            )
        }
        _ => return Err(Error::Invalid),
    };
    if matches!(kind, "key" | "identity") {
        for entry in entries.iter_mut() {
            *entry = key(entry)?;
        }
    }
    if add {
        if !entries.contains(&value) {
            entries.push(value);
        }
    } else {
        let index = entries
            .iter()
            .position(|entry| entry == &value)
            .ok_or(Error::Invalid)?;
        entries.remove(index);
    }
    let (name, contents) = match kind {
        "key" => ("authorized_keys", policy.authorized_keys.join("\n") + "\n"),
        "compiler" => (
            "trusted_compilers",
            policy.trusted_compilers.join("\n") + "\n",
        ),
        "tagger" => ("trusted_taggers", policy.trusted_taggers.join("\n") + "\n"),
        _ => (
            "config",
            serde_json::to_string_pretty(&Config {
                identities: policy.identities,
            })
            .map_err(|_| Error::Invalid)?,
        ),
    };
    let directory = directory()?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)
        .map_err(|error| Error::Configuration(format!("{error}")))?;
    let temporary = directory.join(format!(".{name}-{}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| Error::Configuration(format!("{error}")))?;
    let result = file
        .write_all(contents.as_bytes())
        .and_then(|()| fs::rename(&temporary, directory.join(name)));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| Error::Configuration(format!("{error}")))
}
