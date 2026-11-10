#![no_std]

use fixwasm::{Blob, Error, Handle, Object, Tree, Value, tree};

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    let count = combination.get(2).read_u64()?;
    let mode = combination.get(3).read_u64()?;
    if mode == 7 {
        let saved = combination.get(2);
        let mut result = 0;
        for _ in 0..count {
            result = saved.read_u64()?;
        }
        return Ok(Blob::create(result.to_le_bytes()));
    }
    if mode == 6 {
        drop(0u64.eval()?);
    }
    let mut result = 0;
    for _ in 0..count {
        result = match mode {
            0 => combination.get(2).read_u64()?,
            1 => tree![combination.get(2), combination.get(3)].is_comparable()? as u64,
            2 => {
                drop(combination.get(2).clone());
                1
            }
            3 => count.eval()?.read_u64()?,
            4 => count
                .reference()
                .identification()
                .strict()
                .eval()?
                .read_u64()?,
            5 => {
                let result = tree![0u64, combination.get(4), count]
                    .application()
                    .strict()
                    .eval()?;
                Handle::<Object<Tree>>::try_from(result)?
                    .get(2)
                    .read_u64()?
            }
            6 => combination.len()?,
            _ => unreachable!(),
        };
    }
    Ok(Blob::create(result.to_le_bytes()))
}
