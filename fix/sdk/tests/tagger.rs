#![no_std]
extern crate alloc;
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use fixwasm::{Error, Handle, Object, Tree, Value};

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<Handle<Object<Tree>>, Error> {
    let fields = Handle::<Object<Tree>>::try_from(combination.get(2))?;
    let mode = combination.get(3).read_u64()?;
    if mode == 8 {
        let memory = unsafe { fixwasm::Memory::from_index(3) };
        let bytes = memory.attach_tree_bytes(&fields)?;
        for index in 0..bytes.len() / 32 {
            let mut actual = [0; 32];
            let mut expected = [0; 32];
            bytes.read_into(index * 32, &mut actual)?;
            fields
                .get(index)
                .unsound_handle_bytes()?
                .read_into(0, &mut expected)?;
            assert_eq!(actual, expected);
            assert_eq!(actual, fields.get(index).raw_handle_bytes()?);
        }
        return fields.capture()?.try_into();
    }
    if mode == 6 {
        use fixwasm::ThunkType::*;
        assert_eq!(fields.application().unsound_thunk_type()?, Application);
        assert_eq!(fields.selection().unsound_thunk_type()?, Selection);
        assert_eq!(
            fields.reference().identification().unsound_thunk_type()?,
            Identification
        );
        assert_eq!(
            fields.application().strict().unsound_thunk_type()?,
            Application
        );
        assert_eq!(
            fields.selection().shallow().unsound_thunk_type()?,
            Selection
        );
        return fields.capture()?.try_into();
    }
    if mode == 5 {
        let request = fixwasm::tree![0u64, fields, 5u64, 0u64, 0u64, fixwasm::tree![]];
        let reply = fixwasm::tree![fixwasm::Blob::create(b"callable_invoke"), request].effect()?;
        let reply = Handle::<Object<Tree>>::try_from(reply)?;
        assert_eq!(reply.get(0).read_u64()?, 0);
        return reply.get(1).capture()?.try_into();
    }
    if mode == 4 {
        let claim = fixwasm::tree![1u64, combination.get(3), 0u64].capture()?;
        let claim = Handle::<Object<Tree>>::try_from(claim)?;
        return fixwasm::tree![combination.get(1), claim.reference()]
            .tag()?
            .capture()?
            .try_into();
    }
    if mode == 3 {
        let reply = fixwasm::tree![fixwasm::Blob::create(b"callable_invoke"), fields].effect()?;
        let reply = Handle::<Object<Tree>>::try_from(reply)?;
        assert_eq!(reply.get(0).read_u64()?, 0);
        return reply.get(1).capture()?.try_into();
    }
    if mode == 2 {
        return Tree::tag(
            (0..fields.len()? as usize)
                .map(|index| fixwasm::choose(index == 0, combination.get(1), fields.get(index))),
        )?
        .capture()?
        .try_into();
    }
    if mode == 7 {
        return Tree::tag((0..fields.len()? as usize).map(|index| fields.get(index)))?
            .capture()?
            .try_into();
    }
    if mode != 0 {
        combination.get(4).effect()?;
    }
    let _representation = combination.unsound_handle_bytes()?;
    fields.tag_unchecked()
}
