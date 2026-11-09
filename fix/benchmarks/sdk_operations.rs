#![no_std]

use fix::{Blob, Error, Handle, Object, Tree, Value, tree};

#[fix::apply]
fn apply(combination: &Handle<Object<Tree>, fix::Focus>) -> Result<impl Value, Error> {
    let count = combination.get(2).read_u64()?;
    let mode = combination.get(3).read_u64()?;
    if mode == 7 {
        let mut table = unsafe { fix::Table::from_index(3) };
        let attached = table.attach(combination)?;
        let mut result = 0;
        for _ in 0..count {
            result = attached.get(2).read_u64()?;
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
                fix::invalidate_externrefs()?;
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
                Handle::<Object<Tree>, _>::try_from(result)?
                    .get(2)
                    .read_u64()?
            }
            6 => combination.len()?,
            _ => unreachable!(),
        };
    }
    Ok(Blob::create(result.to_le_bytes()))
}
