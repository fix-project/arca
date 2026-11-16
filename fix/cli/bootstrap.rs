use crate::Error;
use crate::shell::{Blob, Shell, Tree, Value};
use fixrepository::{Repository, RepositoryError};

const MODULES: &[(&str, &[u8])] = &[
    ("fix-parser", include_bytes!(env!("FIX_PARSER_MODULE"))),
    ("collector", include_bytes!(env!("FIX_COLLECTOR_MODULE"))),
    ("serial", include_bytes!(env!("FIX_SERIAL_MODULE"))),
    ("parallel", include_bytes!(env!("FIX_PARALLEL_MODULE"))),
    (
        "distributed",
        include_bytes!(env!("FIX_DISTRIBUTED_MODULE")),
    ),
    ("onepass", include_bytes!(env!("FIX_ONEPASS_MODULE"))),
    ("hint", include_bytes!(env!("FIX_HINT_MODULE"))),
];

pub fn install(shell: &mut Shell, precompile: bool) -> Result<(), Error> {
    if !precompile {
        let repository = Repository::open(&shell.repository())
            .map_err(|error| Error::Backend(format!("{error:?}")))?;
        for &(name, bytes) in MODULES {
            if name == "collector" {
                continue;
            }
            match repository.read_label(name) {
                Ok(_) => continue,
                Err(RepositoryError::MissingLabel { .. }) => (),
                Err(error) => return Err(Error::Backend(format!("{error:?}"))),
            }
            let module = Blob::new(bytes).into_value();
            let request = shell.compilation_request(&module, None)?;
            shell.label(name, &request)?;
        }
        return Ok(());
    }

    let mut requests: Vec<(&str, Value)> = Vec::new();
    for &(name, bytes) in MODULES {
        let module = Blob::new(bytes).into_value();
        let request = shell.compilation_request(&module, None)?;
        requests.push((name, request));
    }
    eprintln!("fix: precompiling {} embedded modules", requests.len());
    let compiled = shell.precompile(
        &requests
            .iter()
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>(),
    )?;
    let compiled = Tree::try_from(compiled)?;
    for ((name, _), runnable) in requests.into_iter().zip(compiled) {
        let runnable = runnable?;
        shell.label(name, &runnable)?;
        if name == "parallel" {
            shell.label("scheduler", &runnable)?;
        }
    }
    Ok(())
}
