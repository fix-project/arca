use clap::Subcommand;
use fixcertificates::linux::{Config, read_config, write_config};
use fixrepository::{Remote, Repository};
use std::{collections::BTreeMap, path::PathBuf};

const PROGRAMS: &[(&str, &str)] = &[
    ("scheduler", "scheduler"),
    ("collector", "collector"),
    ("parser", "fix-parser"),
    ("compiler", "compile"),
];
const KEYS: &[(&str, &str)] = &[
    ("scheduler", "scheduler"),
    ("collector", "collector"),
    ("parser", "fix-parser"),
    ("compiler", "compile"),
    ("pull.mode", "shallow"),
    ("push.mode", "shallow"),
    ("jobs", "auto"),
    ("memory", "16384"),
];

#[derive(Subcommand)]
pub enum ConfigCommand {
    Get { key: String },
    Set { key: String, value: String },
    Unset { key: String },
    List,
}

fn default(key: &str) -> Result<&'static str, Box<dyn std::error::Error>> {
    KEYS.iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| *value)
        .ok_or_else(|| format!("unknown Fix setting {key:?}").into())
}

fn global_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(fixcertificates::linux::directory()?.join("config"))
}

fn local_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(Repository::discover(".")?.path()).join("config"))
}

fn selected<'a>(
    config: &'a Config,
    key: &str,
) -> Result<Option<&'a String>, Box<dyn std::error::Error>> {
    Ok(match key {
        "scheduler" => config.scheduler.as_ref(),
        "collector" => config.collector.as_ref(),
        "parser" => config.parser.as_ref(),
        "compiler" => config.compiler.as_ref(),
        "pull.mode" => config.pull_mode.as_ref(),
        "push.mode" => config.push_mode.as_ref(),
        "jobs" => config.jobs.as_ref(),
        "memory" => config.memory.as_ref(),
        _ => return Err(format!("unknown Fix setting {key:?}").into()),
    })
}

fn select(
    config: &mut Config,
    key: &str,
    value: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(value) = &value {
        match key {
            "jobs" => {
                jobs(value)?;
            }
            "memory" => {
                memory(value)?;
            }
            _ => {}
        }
    }
    if matches!(key, "pull.mode" | "push.mode")
        && let Some(value) = &value
    {
        value.parse::<crate::remotes::Mode>()?;
    }
    *match key {
        "scheduler" => &mut config.scheduler,
        "collector" => &mut config.collector,
        "parser" => &mut config.parser,
        "compiler" => &mut config.compiler,
        "pull.mode" => &mut config.pull_mode,
        "push.mode" => &mut config.push_mode,
        "jobs" => &mut config.jobs,
        "memory" => &mut config.memory,
        _ => return Err(format!("unknown Fix setting {key:?}").into()),
    } = value;
    Ok(())
}

pub fn jobs(value: &str) -> Result<usize, Box<dyn std::error::Error>> {
    if value == "auto" {
        Ok(std::thread::available_parallelism()?.get())
    } else {
        value
            .parse()
            .map_err(|_| "jobs must be a nonnegative integer or auto".into())
    }
}

pub fn memory(value: &str) -> Result<usize, Box<dyn std::error::Error>> {
    value
        .parse::<usize>()
        .ok()
        .filter(|size| *size >= 16 && size.is_power_of_two() && size.checked_mul(1 << 20).is_some())
        .ok_or_else(|| "memory must be a power of two in MiB, at least 16 MiB".into())
}

pub fn settings() -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut config = read_config(&global_path()?)?;
    if let Ok(path) = local_path() {
        let local = read_config(&path)?;
        for (key, _) in KEYS {
            if let Some(value) = selected(&local, key)? {
                select(&mut config, key, Some(value.clone()))?;
            }
        }
    }
    let mut settings = BTreeMap::new();
    for (key, _) in KEYS {
        if let Some(value) = selected(&config, key)? {
            if value.is_empty() {
                return Err(format!("Fix setting {key:?} cannot be empty").into());
            }
            settings.insert((*key).into(), value.clone());
        }
    }
    Ok(settings)
}

pub fn autoconfigure(repository: &Repository) -> Result<(), Box<dyn std::error::Error>> {
    let path = global_path()?;
    let mut config = read_config(&path)?;
    for (key, name) in PROGRAMS {
        select(
            &mut config,
            key,
            Some(repository.read_label(name)?.to_string()),
        )?;
    }
    Ok(write_config(&path, &config)?)
}

pub fn config(
    global: bool,
    local: bool,
    command: ConfigCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = if global {
        Some(global_path()?)
    } else if local {
        Some(local_path()?)
    } else {
        None
    };
    let values = if let Some(path) = &path {
        let config = read_config(path)?;
        let mut values = BTreeMap::new();
        for (key, _) in KEYS {
            if let Some(value) = selected(&config, key)? {
                values.insert((*key).into(), value.clone());
            }
        }
        values
    } else {
        settings()?
    };
    match command {
        ConfigCommand::Get { key } => {
            let fallback = default(&key)?;
            println!(
                "{}",
                values.get(&key).map(String::as_str).unwrap_or(fallback)
            );
        }
        ConfigCommand::Set { key, value } => {
            default(&key)?;
            if value.is_empty() {
                return Err("configuration value cannot be empty".into());
            }
            let path = match path {
                Some(path) => path,
                None => local_path()?,
            };
            let mut config = read_config(&path)?;
            select(&mut config, &key, Some(value))?;
            write_config(&path, &config)?;
        }
        ConfigCommand::Unset { key } => {
            default(&key)?;
            let path = match path {
                Some(path) => path,
                None => local_path()?,
            };
            let mut config = read_config(&path)?;
            select(&mut config, &key, None)?;
            write_config(&path, &config)?;
        }
        ConfigCommand::List => {
            for (key, fallback) in KEYS {
                println!(
                    "{key}\t{}",
                    values.get(*key).map(String::as_str).unwrap_or(fallback)
                );
            }
        }
    }
    Ok(())
}

#[derive(Subcommand)]
pub enum RemoteCommand {
    Add {
        name: String,
        url: String,
    },
    SetUrl {
        name: String,
        url: String,
    },
    SetScheduler {
        name: String,
        scheduler: String,
    },
    SetCollector {
        name: String,
        collector: String,
    },
    GetScheduler {
        name: String,
    },
    GetCollector {
        name: String,
    },
    GetUrl {
        name: String,
    },
    /// List cached labels from a remote; use `fix pull` to refresh them.
    Labels {
        name: String,
    },
    Remove {
        name: String,
    },
    Rename {
        old: String,
        new: String,
    },
    List,
}

pub fn remote(command: RemoteCommand) -> Result<(), Box<dyn std::error::Error>> {
    let repository = Repository::discover(".").map_err(|error| format!("{error:?}"))?;
    let mut remotes = repository.remotes().map_err(|error| format!("{error:?}"))?;
    let action = match command {
        RemoteCommand::Add { name, url } => {
            if remotes.iter().any(|remote| remote.name == name) {
                return Err(format!("remote {name:?} already exists").into());
            }
            let action = format!("added remote {name} at {url}");
            remotes.push(Remote {
                name,
                address: url,
                scheduler: None,
                collector: None,
            });
            action
        }
        RemoteCommand::Rename { old, new } => {
            if remotes.iter().any(|remote| remote.name == new) {
                return Err(format!("remote {new:?} already exists").into());
            }
            let action = format!("renamed remote {old} to {new}");
            remotes
                .iter_mut()
                .find(|remote| remote.name == old)
                .ok_or_else(|| format!("unknown remote {old:?}"))?
                .name = new;
            action
        }
        RemoteCommand::SetUrl { name, url } => {
            let action = format!("set {name} URL to {url}");
            remotes
                .iter_mut()
                .find(|remote| remote.name == name)
                .ok_or_else(|| format!("unknown remote {name:?}"))?
                .address = url;
            action
        }
        RemoteCommand::SetScheduler { name, scheduler } => {
            let action = format!("set {name} scheduler to {scheduler}");
            remotes
                .iter_mut()
                .find(|remote| remote.name == name)
                .ok_or_else(|| format!("unknown remote {name:?}"))?
                .scheduler = Some(scheduler);
            action
        }
        RemoteCommand::SetCollector { name, collector } => {
            let action = format!("set {name} collector to {collector}");
            remotes
                .iter_mut()
                .find(|remote| remote.name == name)
                .ok_or_else(|| format!("unknown remote {name:?}"))?
                .collector = Some(collector);
            action
        }
        RemoteCommand::GetUrl { name } => {
            println!(
                "{}",
                remotes
                    .iter()
                    .find(|remote| remote.name == name)
                    .ok_or_else(|| format!("unknown remote {name:?}"))?
                    .address
            );
            return Ok(());
        }
        RemoteCommand::Labels { name } => {
            if !remotes.iter().any(|remote| remote.name == name) {
                return Err(format!("unknown remote {name:?}").into());
            }
            let refs = repository.remote_refs(&name)?;
            for (label, value) in refs {
                println!("{name}:{label}\t{}", hex::encode(value));
            }
            return Ok(());
        }
        RemoteCommand::GetScheduler { name } => {
            let remote = remotes
                .iter()
                .find(|remote| remote.name == name)
                .ok_or_else(|| format!("unknown remote {name:?}"))?;
            println!("{}", remote.scheduler.as_deref().unwrap_or("scheduler"));
            return Ok(());
        }
        RemoteCommand::GetCollector { name } => {
            let remote = remotes
                .iter()
                .find(|remote| remote.name == name)
                .ok_or_else(|| format!("unknown remote {name:?}"))?;
            println!("{}", remote.collector.as_deref().unwrap_or("collector"));
            return Ok(());
        }
        RemoteCommand::Remove { name } => {
            let index = remotes
                .iter()
                .position(|remote| remote.name == name)
                .ok_or_else(|| format!("unknown remote {name:?}"))?;
            remotes.remove(index);
            format!("removed remote {name}")
        }
        RemoteCommand::List => {
            for remote in remotes {
                println!("{}\t{}", remote.name, remote.address);
            }
            return Ok(());
        }
    };
    repository
        .set_remotes(remotes)
        .map_err(|error| format!("{error:?}"))?;
    common::progress::println(&action);
    Ok(())
}

#[derive(Subcommand)]
pub enum TrustCommand {
    /// Add the agent's Ed25519 keys as signing identities and trusted keys.
    AddAgent,
    AddKey {
        key: String,
    },
    RemoveKey {
        key: String,
    },
    AddCompiler {
        compiler: String,
    },
    AddTagger {
        tagger: String,
    },
    RemoveTagger {
        tagger: String,
    },
    RemoveCompiler {
        compiler: String,
    },
    AddIdentity {
        key: String,
    },
    RemoveIdentity {
        key: String,
    },
    List,
}

pub fn trust(command: TrustCommand) -> Result<(), Box<dyn std::error::Error>> {
    use fixcertificates::linux;
    let (kind, value, add) = match command {
        TrustCommand::AddAgent => {
            let keys = sshagent::identities()?;
            if keys.is_empty() {
                return Err("ssh-agent has no Ed25519 keys".into());
            }
            for key in keys {
                linux::update("key", &key, true)?;
                linux::update("identity", &key, true)?;
            }
            return Ok(());
        }
        TrustCommand::AddKey { key } => ("key", key, true),
        TrustCommand::RemoveKey { key } => ("key", key, false),
        TrustCommand::AddIdentity { key } => ("identity", key, true),
        TrustCommand::RemoveIdentity { key } => ("identity", key, false),
        TrustCommand::AddTagger { tagger } => ("tagger", tagger, true),
        TrustCommand::RemoveTagger { tagger } => ("tagger", tagger, false),
        TrustCommand::AddCompiler { compiler } => ("compiler", compiler, true),
        TrustCommand::RemoveCompiler { compiler } => ("compiler", compiler, false),
        TrustCommand::List => {
            let policy = linux::load()?;
            for compiler in policy.trusted_compilers {
                println!("compiler {compiler}");
            }
            for tagger in policy.trusted_taggers {
                println!("tagger {tagger}");
            }
            for key in policy.authorized_keys {
                println!("key {key}");
            }
            for identity in policy.identities {
                println!("identity {identity}");
            }
            return Ok(());
        }
    };
    let value = if matches!(kind, "compiler" | "tagger") && linux::compiler(&value).is_err() {
        let repository = Repository::discover(".").map_err(|error| format!("{error:?}"))?;
        if value.contains(':') {
            hex::encode(crate::remotes::resolve(&repository, &value)?)
        } else {
            hex::encode(repository.resolve_name(&value)?)
        }
    } else {
        value
    };
    Ok(linux::update(kind, &value, add)?)
}

pub fn remote_argument(value: &str) -> Result<Remote, String> {
    let (name, address) = value.split_once('=').ok_or("expected NAME=URL")?;
    if name.is_empty() || address.is_empty() {
        return Err("expected nonempty NAME=URL".into());
    }
    Ok(Remote {
        name: name.into(),
        address: address.into(),
        scheduler: None,
        collector: None,
    })
}

pub fn remote_name_argument(value: &str) -> Result<(String, String), String> {
    let (remote, name) = value.split_once('=').ok_or("expected REMOTE=FIX_NAME")?;
    if remote.is_empty() || fixcertificates::linux::compiler(name).is_err() {
        return Err("expected REMOTE=FIX_NAME with a canonical Fix name".into());
    }
    Ok((remote.into(), name.into()))
}

pub fn identity_argument(value: &str) -> Result<String, String> {
    fixcertificates::linux::key(value).map_err(|error| error.to_string())
}
