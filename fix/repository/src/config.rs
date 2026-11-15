use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remote {
    pub name: String,
    pub address: String,
}

impl Remote {
    pub fn validate_all(remotes: &[Self]) -> Result<(), RepositoryError> {
        let mut names = hashbrown::HashSet::new();
        for remote in remotes {
            if remote.name.is_empty() || remote.address.is_empty() || !names.insert(&remote.name) {
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
