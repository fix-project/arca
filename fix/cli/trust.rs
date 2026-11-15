use std::{env, fs, io, path::PathBuf};

pub fn load() -> Result<Option<String>, Box<dyn std::error::Error>> {
    let explicit = env::var_os("FIX_TRUST");
    let path = explicit.as_ref().map(PathBuf::from);
    let Some(path) = path else {
        return Ok(None);
    };
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if explicit.is_none() && error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read trust file {}: {error}", path.display()).into()),
    }
}
