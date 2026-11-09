use crate::{Any, Blob, Capture, Error, Focus, Handle, Object, Thunk, Tree, Value};
use alloc::rc::Rc;
use fixapi::{DataType, EncodeType, Fix, Invocation, ValueType};

pub struct Shell {
    combination: Rc<Handle<Any, Capture>>,
}

impl Shell {
    pub fn new(combination: &Handle<Object<Tree>, Focus>) -> Result<Self, Error> {
        Ok(Self {
            combination: Rc::new(combination.capture()?),
        })
    }
}

impl Fix for Shell {
    type Value = Rc<Handle<Any, Capture>>;
    type Error = Error;

    fn advise(&mut self, value: &Self::Value, key: &str, payload: &[u8]) -> Result<(), Error> {
        crate::advise(value.as_ref(), key, payload)
    }
    fn blob(&mut self, bytes: &[u8]) -> Result<Self::Value, Error> {
        Ok(Rc::new(Blob::create(bytes).capture()?))
    }
    fn tree(&mut self, children: &[Self::Value]) -> Result<Self::Value, Error> {
        Ok(Rc::new(Tree::create(children).capture()?))
    }
    fn get(&mut self, tree: &Self::Value, index: usize) -> Result<Self::Value, Error> {
        let tree = Handle::<Object<Tree>, _>::try_from(tree.as_ref())?;
        Ok(Rc::new(tree.get(index).capture()?))
    }
    fn read(&mut self, blob: &Self::Value, offset: usize, bytes: &mut [u8]) -> Result<(), Error> {
        blob.read_at(None, offset, bytes)
    }
    fn len(&mut self, value: &Self::Value) -> Result<u64, Error> {
        value.len()
    }
    fn value_type(&mut self, value: &Self::Value) -> Result<ValueType, Error> {
        value.value_type()
    }
    fn data_type(&mut self, value: &Self::Value) -> Result<DataType, Error> {
        value.data_type()
    }
    fn encode_type(&mut self, value: &Self::Value) -> Result<EncodeType, Error> {
        value.encode_type()
    }
    fn is_tag(&mut self, value: &Self::Value) -> Result<bool, Error> {
        value.is_tag()
    }
    fn is_comparable(&mut self, value: &Self::Value) -> Result<bool, Error> {
        value.is_comparable()
    }
    fn equals(&mut self, left: &Self::Value, right: &Self::Value) -> Result<bool, Error> {
        left.equals(right)
    }
    fn aliases(&self, left: &Self::Value, right: &Self::Value) -> bool {
        Rc::ptr_eq(left, right)
    }
    fn reference(&mut self, value: &Self::Value) -> Result<Self::Value, Error> {
        Ok(Rc::new(
            Handle::<Object<Any>, _>::try_from(value.as_ref())?
                .reference()
                .capture()?,
        ))
    }
    fn identification(&mut self, value: &Self::Value) -> Result<Self::Value, Error> {
        Ok(Rc::new(value.identification().capture()?))
    }
    fn application(&mut self, value: &Self::Value) -> Result<Self::Value, Error> {
        Ok(Rc::new(value.application().capture()?))
    }
    fn selection(&mut self, value: &Self::Value) -> Result<Self::Value, Error> {
        Ok(Rc::new(value.selection().capture()?))
    }
    fn strict(&mut self, value: &Self::Value) -> Result<Self::Value, Error> {
        Ok(Rc::new(
            Handle::<Thunk, _>::try_from(value.as_ref())?
                .strict()
                .capture()?,
        ))
    }
    fn shallow(&mut self, value: &Self::Value) -> Result<Self::Value, Error> {
        Ok(Rc::new(
            Handle::<Thunk, _>::try_from(value.as_ref())?
                .shallow()
                .capture()?,
        ))
    }
    fn eval(&mut self, value: &Self::Value) -> Result<Self::Value, Error> {
        Ok(Rc::new(value.eval()?))
    }
}

impl Invocation for Shell {
    fn combination(&mut self) -> Result<Self::Value, Error> {
        Ok(self.combination.clone())
    }
    fn tag(&mut self, fields: &[Self::Value]) -> Result<Self::Value, Error> {
        Ok(Rc::new(Tree::create(fields).tag()?.capture()?))
    }
    fn effect(&mut self, combination: &Self::Value) -> Result<Self::Value, Error> {
        Ok(Rc::new(combination.effect()?))
    }
}
