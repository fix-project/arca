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
                || remote.name.contains(':')
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
    fn remote_refs_path(&self, remote: &str) -> Result<String, RepositoryError> {
        if remote.is_empty() {
            return Err(RepositoryError::InvalidLabel);
        }
        Ok(format!("{}/remote_refs/{}", self.path, hex::encode(remote)))
    }

    pub fn remote_refs(&self, remote: &str) -> Result<Vec<(String, [u8; 32])>, RepositoryError> {
        let path = self.remote_refs_path(remote)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(ErrorKind::NotFound) => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let refs: Vec<(String, [u8; 32])> =
            serde_json::from_slice(&bytes).map_err(|_| RepositoryError::InvalidRepository)?;
        self.validate_remote_refs(&refs)?;
        Ok(refs)
    }

    fn validate_remote_refs(&self, refs: &[(String, [u8; 32])]) -> Result<(), RepositoryError> {
        let mut names = hashbrown::HashSet::new();
        for (label, name) in refs {
            self.label_path(label)?;
            if !names.insert(label) || !Handle::parse_ref(name)?.is_canonical() {
                return Err(RepositoryError::InvalidRepository);
            }
        }
        Ok(())
    }

    pub fn set_remote_refs(
        &self,
        remote: &str,
        refs: Vec<(String, [u8; 32])>,
    ) -> Result<(), RepositoryError> {
        self.validate_remote_refs(&refs)?;
        let path = self.remote_refs_path(remote)?;
        match fs::mkdir(&format!("{}/remote_refs", self.path)) {
            Ok(()) | Err(ErrorKind::AlreadyExists) => (),
            Err(error) => return Err(error.into()),
        }
        self.write_atomic(
            &path,
            &serde_json::to_vec(&refs).map_err(|_| RepositoryError::InvalidRepository)?,
        )
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
