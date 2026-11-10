#![no_std]
extern crate alloc;

use fixwasm::{Blob, Error, Handle, Object, Tree, Value, tree};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use alloc::vec::Vec;

fn upper_bound(keys: &[i32], key: i32) -> usize {
    keys.partition_point(|&k| k <= key)
}

#[fixwasm::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>>,
) -> Result<impl Value<Type = fixwasm::Any> + 'a, Error> {
    if combination.len()? != 6 {
        return Err(Error::WrongType);
    }
    let arguments = [
        combination.get(1),
        combination.get(2),
        combination.get(3),
        combination.get(4),
        combination.get(5),
    ];
    let [function, keys_h, childrenordata_h, key_h, n_h] = arguments;

    let key = i32::from_le_bytes(
        Handle::<Object<Blob>>::try_from(&key_h)?
            .read()?
            .try_into()
            .map_err(|_| Error::WrongType)?,
    );
    let keys_data = Handle::<Object<Blob>>::try_from(keys_h)?.read()?;
    let isleaf = *keys_data.first().ok_or(Error::WrongType)? != 0;
    let (key_bytes, _) = keys_data[1..].as_chunks();
    let keys: Vec<i32> = key_bytes.iter().copied().map(i32::from_le_bytes).collect();

    let idx = upper_bound(&keys, key);
    use fixwasm::Choice::{Left, Right};
    Ok(if isleaf {
        if idx != 0 && keys[idx - 1] == key {
            let n = Handle::<Object<Blob>>::try_from(n_h)?.read_u64()? as usize;
            let data = Handle::<Object<Tree>>::try_from(childrenordata_h)?;
            let last = data.len()? - 1;
            let mut tail = data.erase();
            let mut selections = Vec::with_capacity(n);
            for i in 0..n {
                selections.push(
                    tree![tail.clone(), if i == 0 { idx as u64 } else { 1u64 }, last]
                        .reference()
                        .selection(),
                );
                tail = tree![tail, last].reference().selection().shallow().erase();
            }
            Left(Tree::create(selections).erase())
        } else {
            Right(Left(fixwasm::tree![].erase()))
        }
    } else {
        let child = tree![childrenordata_h, idx as u64 + 1]
            .reference()
            .selection()
            .strict();
        let child_keys = tree![child, 0u64].reference().selection().strict();
        Right(Right(
            tree![Blob::create(b""), function, child_keys, child, key_h, n_h]
                .reference()
                .application()
                .erase(),
        ))
    })
}
