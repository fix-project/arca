use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remote {
    pub name: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collector: Option<String>,
}

impl Remote {
    pub fn validate_all(remotes: &[Self]) -> Result<(), RepositoryError> {
        let mut names = hashbrown::HashSet::new();
        for remote in remotes {
            if remote.name.is_empty()
                || remote.name.contains([':', '/', '\0'])
                || remote.address.is_empty()
                || remote.scheduler.as_deref() == Some("")
                || remote.collector.as_deref() == Some("")
                || !names.insert(&remote.name)
            {
                return Err(RepositoryError::InvalidLabel);
            }
        }
        Ok(())
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default)]
    remotes: Vec<Remote>,
}

impl Repository {
    pub fn remote_refs(&self, remote: &str) -> Result<Vec<(String, [u8; 32])>, RepositoryError> {
        let prefix = format!("{remote}:");
        self.labels()?
            .into_iter()
            .filter_map(|label| {
                let suffix = label.strip_prefix(&prefix)?.into();
                Some(
                    self.read_label(&label)
                        .map(|value| (suffix, *value.as_bytes())),
                )
            })
            .collect()
    }

    pub fn set_remote_refs(
        &self,
        remote: &str,
        refs: Vec<(String, [u8; 32])>,
    ) -> Result<(), RepositoryError> {
        let mut names = hashbrown::HashSet::new();
        let mut values = Vec::new();
        for (label, name) in refs {
            self.label_path(&label)?;
            if !names.insert(label.clone()) {
                return Err(RepositoryError::InvalidRepository);
            }
            let view = Handle::parse_ref(&name)?;
            if !view.is_canonical() {
                return Err(RepositoryError::NonCanonical);
            }
            let value = unsafe { view.to_owned() };
            let value = match value.view() {
                HandleView::Object(object) => object.into_ref().into(),
                _ => value,
            };
            values.push((format!("{remote}:{label}"), value));
        }
        for (label, _) in self.remote_refs(remote)? {
            if !names.contains(&label) {
                self.remove_label(&format!("{remote}:{label}"))?;
            }
        }
        for (label, value) in values {
            self.write_label(&label, value)?;
        }
        Ok(())
    }

    pub fn remotes(&self) -> Result<Vec<Remote>, RepositoryError> {
        let bytes = match fs::read(&format!("{}/remotes", self.path)) {
            Ok(bytes) => bytes,
            Err(ErrorKind::NotFound) => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let config: Config =
            serde_json::from_slice(&bytes).map_err(|_| RepositoryError::InvalidRepository)?;
        Remote::validate_all(&config.remotes).map_err(|_| RepositoryError::InvalidRepository)?;
        Ok(config.remotes)
    }

    pub fn set_remotes(&self, remotes: Vec<Remote>) -> Result<(), RepositoryError> {
        Remote::validate_all(&remotes)?;
        self.write_atomic(
            &format!("{}/remotes", self.path),
            &serde_json::to_vec_pretty(&Config { remotes })
                .map_err(|_| RepositoryError::InvalidRepository)?,
        )
    }
}
