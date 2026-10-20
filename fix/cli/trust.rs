use std::{env, fs, io, path::PathBuf};

pub fn load() -> Result<String, Box<dyn std::error::Error>> {
    let explicit = env::var_os("FIX_TRUST");
    let path = explicit.as_ref().map(PathBuf::from).or_else(|| {
        env::var_os("XDG_CONFIG_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|directory| directory.join("fix/trust"))
    });
    let Some(path) = path else {
        return Ok(String::new());
    };
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(contents),
        Err(error) if explicit.is_none() && error.kind() == io::ErrorKind::NotFound => {
            Ok(String::new())
        }
        Err(error) => Err(format!("cannot read trust file {}: {error}", path.display()).into()),
    }
}
