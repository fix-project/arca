use super::*;
#[doc(hidden)]
pub struct Path<'a> {
    pub(super) index: usize,
    pub(super) next: Option<&'a Path<'a>>,
}
pub trait Lookup {
    #[doc(hidden)]
    fn load(&self, path: Option<&Path<'_>>) -> Result<(), Error>;
}
impl<P: Lookup + ?Sized> Lookup for &P {
    fn load(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        (**self).load(path)
    }
}
impl<P: Lookup + ?Sized> Lookup for &mut P {
    fn load(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        (**self).load(path)
    }
}
pub struct Focus {
    pub(super) _private: (),
}
impl Lookup for Focus {
    fn load(&self, mut path: Option<&Path<'_>>) -> Result<(), Error> {
        unsafe { fix_sdk_root() };
        while let Some(entry) = path {
            status(unsafe { fix_sdk_entry(entry.index) })?;
            path = entry.next;
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub struct Child<P> {
    pub(super) parent: P,
    pub(super) index: usize,
}
impl<P: Lookup> Lookup for Child<P> {
    fn load(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.parent.load(Some(&Path {
            index: self.index,
            next: path,
        }))
    }
}
pub struct Handle<T = Any, R = Focus> {
    pub(super) location: R,
    kind: PhantomData<T>,
}
impl<T, R: Clone> Clone for Handle<T, R> {
    fn clone(&self) -> Self {
        Self::new(self.location.clone())
    }
}
impl<T, R: Copy> Copy for Handle<T, R> {}
impl<T: FixType, R: Lookup, V: Value> PartialEq<V> for Handle<T, R> {
    fn eq(&self, other: &V) -> bool {
        equal(self, other)
    }
}
impl<T, R> Handle<T, R> {
    pub(super) fn new(location: R) -> Self {
        Self {
            location,
            kind: PhantomData,
        }
    }
}
macro_rules! narrowing {
    ($($target:ty),* $(,)?) => {$(
        impl<R: Lookup> TryFrom<Handle<Any, R>> for Handle<$target, R> {
            type Error = Error;
            fn try_from(value: Handle<Any, R>) -> Result<Self, Error> {
                value.checked()
            }
        }
        impl<'a, R: Lookup> TryFrom<&'a Handle<Any, R>> for Handle<$target, &'a R> {
            type Error = Error;
            fn try_from(value: &'a Handle<Any, R>) -> Result<Self, Error> {
                Handle::<Any, _>::new(&value.location).checked()
            }
        }
        impl<R> From<Handle<$target, R>> for Handle<Any, R> {
            fn from(value: Handle<$target, R>) -> Self { Self::new(value.location) }
        }
    )*};
}
impl<R: Lookup> Handle<Any, R> {
    pub(crate) fn checked<T: FixType>(self) -> Result<Handle<T, R>, Error> {
        if let Some(kind) = T::VALUE
            && self.value_type()? != kind
        {
            return Err(Error::WrongType);
        }
        if let Some(kind) = T::DATA
            && self.data_type()? != kind
        {
            return Err(Error::WrongType);
        }
        Ok(Handle::new(self.location))
    }
}
narrowing!(
    Object<Any>,
    Object<Blob>,
    Object<Tree>,
    Ref<Any>,
    Ref<Blob>,
    Ref<Tree>,
    Thunk,
    Encode
);
impl<T: FixType, R: Lookup> Value for Handle<T, R> {
    type Type = T;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.location.load(path)
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        self.location.load(path)?;
        current_value_type()
    }
    fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
        self.location.load(path)?;
        current_data_type()
    }
    fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
        self.location.load(path)?;
        current_encode_type()
    }
    fn len_at(&self, path: Option<&Path<'_>>) -> Result<usize, Error> {
        self.location.load(path)?;
        query_current(fix_sdk_len).map(|n| n as usize)
    }
    fn is_tag_at(&self, path: Option<&Path<'_>>) -> Result<bool, Error> {
        self.location.load(path)?;
        query_current(fix_sdk_is_tag).map(|n| n != 0)
    }
    fn read_at(
        &self,
        path: Option<&Path<'_>>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        self.location.load(path)?;
        status(unsafe { fix_sdk_read(offset, bytes.as_mut_ptr(), bytes.len()) })
    }
}
impl<T: FixType, R: Lookup> Handle<T, R> {
    pub fn with<'a, F, V>(&'a mut self, index: usize, function: F) -> Result<V, Error>
    where
        F: FnOnce(Handle<Any, Child<&'a mut R>>) -> Result<V, Error>,
    {
        let child = Handle::new(Child {
            parent: &mut self.location,
            index,
        });
        child.value_type()?;
        function(child)
    }
}
