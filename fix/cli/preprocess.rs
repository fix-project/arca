#[path = "../runtime/src/preprocessor.rs"]
mod preprocessor;

use std::{
    fs,
    io::{Read, Write},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let mut imports = None;
    let mut script = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--imports" => imports = Some(arguments.next().ok_or("expected manifest path")?),
            _ if script.is_none() => script = Some(argument),
            _ => return Err("unexpected argument".into()),
        }
    }
    let source = if let Some(path) = script {
        if path == "-" {
            let mut source = String::new();
            std::io::stdin().read_to_string(&mut source)?;
            source
        } else {
            fs::read_to_string(path)?
        }
    } else {
        return Err("expected script".into());
    };
    let source = if let Some(path) = imports {
        let mut manifest = fs::File::create(path)?;
        let mut files = std::collections::BTreeMap::new();
        let mut next = 0;
        preprocessor::Preprocessor::new(&source).preprocess_with(|path| {
            if let Some(index) = files.get(path) {
                return Ok(format!("$file{index}"));
            }
            while source.contains(&format!("$file{next}")) {
                next += 1;
            }
            let index = next;
            next += 1;
            files.insert(path.to_owned(), index);
            write!(manifest, "file{index}\0{path}\0").map_err(|error| error.to_string())?;
            Ok(format!("$file{index}"))
        })?
    } else {
        preprocessor::Preprocessor::new(&source)
            .preprocess(|path| fs::read(path).map_err(|error| error.to_string()))?
    };
    std::io::stdout().lock().write_all(source.as_bytes())?;
    Ok(())
}
