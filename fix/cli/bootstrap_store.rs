use crate::{Error, error};
use std::path::PathBuf;

pub fn repository() -> Result<PathBuf, Error> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(|| Error::Backend("HOME is not set".into()))?;
    let data = if data.is_absolute() {
        data
    } else {
        std::env::current_dir().map_err(error)?.join(data)
    };
    Ok(data.join("fix/bootstrap/.fix"))
}
