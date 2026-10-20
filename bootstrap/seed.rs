use fixhandle::{Blob, Handle, Name, Tree};
use std::{env, fs, os::unix::fs::symlink, path::PathBuf};

struct Repository(PathBuf);

impl Repository {
    fn publish(&self, object: Handle, bytes: &[u8]) -> std::io::Result<Handle> {
        if matches!(object.name(), Name::Literal(_)) {
            return Ok(object);
        }
        let mut key = *object.as_bytes();
        key[30..].fill(0xff);
        let key = hex::encode(key);
        let data = self.0.join("data").join(&key);
        if !data.try_exists()? {
            fs::write(data, bytes)?;
        }
        let record = self.0.join("objects").join(object.to_string());
        let target = PathBuf::from(format!("../data/{key}"));
        match symlink(&target, &record) {
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                assert_eq!(fs::read_link(record)?, target);
            }
            result => result?,
        }
        if let Ok(tree) = Tree::try_from(object)
            && tree.is_tag()
        {
            let mut path = self.0.join("tags");
            let fields = bytes.as_chunks::<32>().0;
            let count = fields.len().saturating_sub(1).min(8);
            for field in fields.iter().skip(1).take(8) {
                path.push(hex::encode(field));
            }
            path.push("entries");
            fs::create_dir_all(&path)?;
            let record = path.join(object.to_string());
            let target = PathBuf::from(format!("{}objects/{object}", "../".repeat(count + 2)));
            match symlink(&target, &record) {
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    assert_eq!(fs::read_link(record)?, target);
                }
                result => result?,
            }
        }
        Ok(object)
    }

    fn blob(&self, bytes: &[u8]) -> std::io::Result<Handle> {
        self.publish(Blob::canonical(bytes).unwrap().into(), bytes)
    }

    fn tree(&self, fields: &[Handle], tag: bool) -> std::io::Result<Handle> {
        let tree = Tree::canonical(fields).unwrap();
        let object = if tag { tree.tag_descriptor() } else { tree };
        let bytes: Vec<_> = fields.iter().flat_map(|field| *field.as_bytes()).collect();
        self.publish(object.into(), &bytes)
    }

    fn tag(&self, author: Handle, body: Handle, claim: &str) -> std::io::Result<Handle> {
        let authors = self.tree(&[author], false)?;
        self.tree(&[authors, body, self.blob(claim.as_bytes())?], true)
    }

    fn label(&self, name: &str, value: Handle) -> std::io::Result<()> {
        assert!(!name.is_empty() && name != "." && name != ".." && !name.contains('/'));
        fs::write(self.0.join("labels").join(name), format!("{value}\n"))
    }

    fn lookup(&self, name: &str) -> std::io::Result<Handle> {
        let mut bytes = [0; 32];
        hex::decode_to_slice(name.trim(), &mut bytes).expect("expected a handle");
        let view = Handle::parse_ref(&bytes).expect("invalid handle");
        assert!(view.is_canonical());
        let mut key = bytes;
        key[30..].fill(0xff);
        let record = self.0.join("objects").join(name.trim());
        assert_eq!(
            fs::read_link(&record)?,
            PathBuf::from(format!("../data/{}", hex::encode(key)))
        );
        assert_eq!(fs::metadata(record)?.len(), view.byte_len());
        // The exact Object record in this trusted repository establishes provenance.
        Ok(unsafe { view.to_owned() })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    let repository = Repository(PathBuf::from(&args[1]));
    match args[0].as_str() {
        "seed" => {
            assert!(args.len() >= 3 && (args.len() - 3) % 2 == 0);
            let compiler = repository.blob(&fs::read(&args[2])?)?;
            let fixpoint = repository.tag(compiler, compiler, "Runnable")?;
            repository.label("compile", compiler)?;
            repository.label("fixpoint", fixpoint)?;
            for pair in args[3..].chunks_exact(2) {
                let executable = repository.blob(&fs::read(&pair[1])?)?;
                repository.label(&pair[0], repository.tag(compiler, executable, "Runnable")?)?;
            }
            fs::write(repository.0.join("trust"), format!("{compiler}\n"))?;
        }
        "tag" => {
            assert_eq!(args.len(), 4);
            let author =
                repository.lookup(&fs::read_to_string(repository.0.join("labels/compile"))?)?;
            let body = repository.lookup(&args[3])?;
            println!("{}", repository.tag(author, body, &args[2])?);
        }
        _ => return Err("expected seed or tag".into()),
    }
    Ok(())
}
