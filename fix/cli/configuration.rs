use clap::Subcommand;
use fixrepository::{Remote, Repository};

#[derive(Subcommand)]
pub enum RemoteCommand {
    Add { name: String, url: String },
    SetUrl { name: String, url: String },
    GetUrl { name: String },
    Remove { name: String },
    Rename { old: String, new: String },
    List,
}

pub fn remote(command: RemoteCommand) -> Result<(), Box<dyn std::error::Error>> {
    let repository = Repository::discover(".").map_err(|error| format!("{error:?}"))?;
    let mut remotes = repository.remotes().map_err(|error| format!("{error:?}"))?;
    match command {
        RemoteCommand::Add { name, url } => {
            if remotes.iter().any(|remote| remote.name == name) {
                return Err(format!("remote {name:?} already exists").into());
            }
            remotes.push(Remote { name, address: url });
        }
        RemoteCommand::Rename { old, new } => {
            if remotes.iter().any(|remote| remote.name == new) {
                return Err(format!("remote {new:?} already exists").into());
            }
            remotes
                .iter_mut()
                .find(|remote| remote.name == old)
                .ok_or_else(|| format!("unknown remote {old:?}"))?
                .name = new;
        }
        RemoteCommand::SetUrl { name, url } => {
            remotes
                .iter_mut()
                .find(|remote| remote.name == name)
                .ok_or_else(|| format!("unknown remote {name:?}"))?
                .address = url;
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
        RemoteCommand::Remove { name } => {
            let index = remotes
                .iter()
                .position(|remote| remote.name == name)
                .ok_or_else(|| format!("unknown remote {name:?}"))?;
            remotes.remove(index);
        }
        RemoteCommand::List => {
            for remote in remotes {
                println!("{}\t{}", remote.name, remote.address);
            }
            return Ok(());
        }
    }
    repository
        .set_remotes(remotes)
        .map_err(|error| format!("{error:?}").into())
}

#[derive(Subcommand)]
pub enum TrustCommand {
    AddKey { key: String },
    RemoveKey { key: String },
    AddCompiler { compiler: String },
    AddTagger { tagger: String },
    RemoveTagger { tagger: String },
    RemoveCompiler { compiler: String },
    AddIdentity { key: String },
    RemoveIdentity { key: String },
    List,
}

pub fn trust(command: TrustCommand) -> Result<(), Box<dyn std::error::Error>> {
    use fixcertificates::linux;
    let (kind, value, add) = match command {
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
        repository
            .read_label(&value)
            .map_err(|error| format!("{error:?}"))?
            .to_string()
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
    })
}

pub fn identity_argument(value: &str) -> Result<String, String> {
    fixcertificates::linux::key(value).map_err(|error| error.to_string())
}
