use crate::{Blob, Cast, Error, Object, Select, Tree, Value};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub dev: u64,
    pub ino: u64,
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
) -> Result<Select<&impl Value<Type = Object<Tree>>>, Error> {
    if descriptor.len()? != 3 {
        return Err(Error::WrongType);
    }
    Ok(Value::get(descriptor, index))
}
pub fn name(
    descriptor: &impl Value<Type = Object<Tree>>,
) -> Result<impl Value<Type = Object<Blob>> + Copy + '_, Error> {
    let name: Cast<Object<Blob>, _> = field(descriptor, 0)?.try_into()?;
    Ok(name)
}
pub fn stat(
    descriptor: &impl Value<Type = Object<Tree>>,
) -> Result<Select<&impl Value<Type = Object<Tree>>>, Error> {
    field(descriptor, 1)
}
pub fn contents(
    descriptor: &impl Value<Type = Object<Tree>>,
) -> Result<Select<&impl Value<Type = Object<Tree>>>, Error> {
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
) -> impl Value<Type = Object<Tree>> {
    crate::tree![name, stat, contents]
}
pub fn read_stat(object: &impl Value<Type = Object<Tree>>) -> Result<Stat, Error> {
    if object.len()? != 8 {
        return Err(Error::WrongType);
    }
    let mut fields = [0u64; 8];
    for (index, field) in fields.iter_mut().enumerate() {
        let value = Cast::<Object<Blob>, _>::try_from(Value::get(object, index))?;
        *field = if index == 2 {
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
        ino: fields[1],
        filetype: fields[2] as u8,
        nlink: fields[3],
        size: fields[4],
        atim: fields[5],
        mtim: fields[6],
        ctim: fields[7],
    })
}
pub fn write_stat(stat: &Stat) -> impl Value<Type = Object<Tree>> + Clone + use<> {
    crate::tree![
        stat.dev,
        stat.ino,
        Blob::create([stat.filetype]),
        stat.nlink,
        stat.size,
        stat.atim,
        stat.mtim,
        stat.ctim
    ]
}
