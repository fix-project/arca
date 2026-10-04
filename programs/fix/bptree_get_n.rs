#![no_std]
extern crate alloc;

use fix::{Blob, Error, Handle, Object, Tree, Value, tree};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use alloc::{rc::Rc, vec::Vec};

fn upper_bound(keys: &[i32], key: i32) -> usize {
    keys.partition_point(|&k| k <= key)
}

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = fix::Any> + 'a, Error> {
    if combination.len()? != 5 {
        return Err(Error::WrongType);
    }
    let arguments = [
        combination.get(0),
        combination.get(1),
        combination.get(2),
        combination.get(3),
        combination.get(4),
    ];
    let [_, keys_h, childrenordata_h, key_h, n_h] = arguments;

    let key = i32::from_le_bytes(
        Handle::<Object<Blob>, _>::try_from(key_h)?
            .read()?
            .try_into()
            .map_err(|_| Error::WrongType)?,
    );
    let keys_data = Handle::<Object<Blob>, _>::try_from(keys_h)?.read()?;
    let isleaf = *keys_data.first().ok_or(Error::WrongType)? != 0;
    let (key_bytes, _) = keys_data[1..].as_chunks();
    let keys: Vec<i32> = key_bytes.iter().copied().map(i32::from_le_bytes).collect();

    let idx = upper_bound(&keys, key);
    use fix::Choice::{Left, Right};
    Ok(if isleaf {
        if idx != 0 && keys[idx - 1] == key {
            let n = Handle::<Object<Blob>, _>::try_from(n_h)?.read_u64()? as usize;
            let data = Handle::<Object<Tree>, _>::try_from(childrenordata_h)?;
            let last = data.len()? as u64 - 1;
            let mut tail: Rc<dyn Value<Type = fix::Any> + 'a> = Rc::new(data.erase());
            let mut selections = Vec::with_capacity(n);
            for i in 0..n {
                selections.push(
                    tree![tail.clone(), if i == 0 { idx as u64 } else { 1u64 }, last]
                        .reference()
                        .selection(),
                );
                tail = Rc::new(tree![tail, last].reference().selection().shallow().erase());
            }
            Left(Tree::create(selections).erase())
        } else {
            Right(Left(fix::tree![].erase()))
        }
    } else {
        let child = tree![childrenordata_h, idx as u64 + 1]
            .reference()
            .selection()
            .shallow();
        let child_keys = tree![child, 0u64].reference().selection().strict();
        Right(Right(
            tree![arguments[0], child_keys, child, key_h, n_h]
                .reference()
                .application()
                .erase(),
        ))
    })
}
