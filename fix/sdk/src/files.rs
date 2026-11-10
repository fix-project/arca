use crate::{Blob, Error, Handle, Object, Tree, Value};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub dev: u64,
    pub filetype: u8,
    pub nlink: u64,
    pub size: u64,
    pub atim: u64,
    pub mtim: u64,
    pub ctim: u64,
}

fn field(
    descriptor: &impl Value<Type = Object<Tree>>,
    index: usize,
) -> Result<Handle<crate::Any>, Error> {
    if descriptor.len()? != 3 {
        return Err(Error::WrongType);
    }
    Ok(Value::get(descriptor, index))
}
pub fn name(descriptor: &impl Value<Type = Object<Tree>>) -> Result<Handle<Object<Blob>>, Error> {
    let name: Handle<Object<Blob>> = field(descriptor, 0)?.try_into()?;
    Ok(name)
}
pub fn stat(descriptor: &impl Value<Type = Object<Tree>>) -> Result<Handle<crate::Any>, Error> {
    field(descriptor, 1)
}
pub fn contents(descriptor: &impl Value<Type = Object<Tree>>) -> Result<Handle<crate::Any>, Error> {
    field(descriptor, 2)
}
pub fn make_file<
    N: Value<Type = Object<Blob>>,
    S: Value<Type = crate::Any>,
    C: Value<Type = crate::Any>,
>(
    name: N,
    stat: S,
    contents: C,
) -> Handle<Object<Tree>> {
    crate::tree![name, stat, contents]
}
pub fn read_stat(object: &impl Value<Type = Object<Tree>>) -> Result<Stat, Error> {
    if object.len()? != 7 {
        return Err(Error::WrongType);
    }
    let mut fields = [0u64; 7];
    for (index, field) in fields.iter_mut().enumerate() {
        let value = Handle::<Object<Blob>>::try_from(Value::get(object, index))?;
        *field = if index == 1 {
            if value.len()? != 1 {
                return Err(Error::WrongType);
            }
            let mut bytes = [0];
            value.read_into(0, &mut bytes)?;
            u64::from(bytes[0])
        } else {
            value.read_u64()?
        };
    }
    Ok(Stat {
        dev: fields[0],
        filetype: fields[1] as u8,
        nlink: fields[2],
        size: fields[3],
        atim: fields[4],
        mtim: fields[5],
        ctim: fields[6],
    })
}
pub fn write_stat(stat: &Stat) -> Handle<Object<Tree>> {
    crate::tree![
        stat.dev,
        Blob::create([stat.filetype]),
        stat.nlink,
        stat.size,
        stat.atim,
        stat.mtim,
        stat.ctim
    ]
}
