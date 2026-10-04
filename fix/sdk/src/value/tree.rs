use super::*;

impl<R: Lookup> Handle<Object<Tree>, R> {
    pub fn iter(&self) -> Result<impl Iterator<Item = Handle<Any, Child<&R>>> + Clone, Error> {
        Ok((0..self.len()?).map(move |index| {
            Handle::new(Child {
                parent: &self.location,
                index,
            })
        }))
    }
    pub fn children(&self) -> Result<Vec<Handle<Any, Child<&R>>>, Error> {
        Ok(self.iter()?.collect())
    }
}

impl<'a, R: Lookup + ?Sized> Handle<Object<Tree>, &'a R> {
    pub fn get(self, index: usize) -> Handle<Any, Child<&'a R>> {
        Handle::new(Child {
            parent: self.location,
            index,
        })
    }
}
impl<R: Lookup + Copy> Handle<Object<Tree>, Child<R>> {
    pub fn get(self, index: usize) -> Handle<Any, Child<Child<R>>> {
        Handle::new(Child {
            parent: self.location,
            index,
        })
    }
}
impl Handle<Object<Tree>, Focus> {
    pub(crate) fn combination() -> Self {
        Self::new(Focus { _private: () })
    }
}
impl Handle<Object<Tree>, Focus> {
    pub fn get(&self, index: usize) -> Handle<Any, Child<&Focus>> {
        Handle::new(Child {
            parent: &self.location,
            index,
        })
    }
}

pub trait Children {
    type Child<'a>: Value<Type = Any>
    where
        Self: 'a;
    fn len(&self) -> usize;
    fn child(&self, index: usize) -> Result<Self::Child<'_>, Error>;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Nil;
impl Children for Nil {
    type Child<'a> = Erased<u64>;
    fn len(&self) -> usize {
        0
    }
    fn child(&self, _: usize) -> Result<Self::Child<'_>, Error> {
        Err(Error::OutOfBounds)
    }
}
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Cons<V, C>(pub V, pub C);
impl<V: Value, C: Children> Children for Cons<V, C> {
    type Child<'a>
        = Choice<Erased<&'a V>, C::Child<'a>>
    where
        Self: 'a;
    fn len(&self) -> usize {
        1 + self.1.len()
    }
    fn child(&self, index: usize) -> Result<Self::Child<'_>, Error> {
        if index == 0 {
            Ok(Choice::Left((&self.0).erase()))
        } else {
            Ok(Choice::Right(self.1.child(index - 1)?))
        }
    }
}
#[derive(Clone)]
pub struct Iter<I>(I);
impl<I: Iterator + Clone> Children for Iter<I>
where
    I::Item: Value,
{
    type Child<'a>
        = Erased<I::Item>
    where
        Self: 'a;
    fn len(&self) -> usize {
        let (min, max) = self.0.size_hint();
        if max == Some(min) {
            min
        } else {
            self.0.clone().count()
        }
    }
    fn child(&self, index: usize) -> Result<Self::Child<'_>, Error> {
        Ok(self.0.clone().nth(index).ok_or(Error::OutOfBounds)?.erase())
    }
}
pub struct Slice<V, C>(C, PhantomData<V>);
impl<V, C: Clone> Clone for Slice<V, C> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}
impl<V: Value, C: AsRef<[V]>> Children for Slice<V, C> {
    type Child<'a>
        = Erased<&'a V>
    where
        Self: 'a;
    fn len(&self) -> usize {
        self.0.as_ref().len()
    }
    fn child(&self, index: usize) -> Result<Self::Child<'_>, Error> {
        Ok(self
            .0
            .as_ref()
            .get(index)
            .ok_or(Error::OutOfBounds)?
            .erase())
    }
}
#[derive(Clone, Copy)]
pub struct CreateTree<C> {
    children: C,
    tag: bool,
}
impl<C: Children> CreateTree<C> {
    fn query<T>(
        &self,
        path: Option<&Path<'_>>,
        root: impl FnOnce(&Self) -> Result<T, Error>,
        child: impl FnOnce(C::Child<'_>, Option<&Path<'_>>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if let Some(entry) = path {
            child(self.children.child(entry.index)?, entry.next)
        } else if self.tag && self.children.is_empty() {
            Err(Error::OutOfBounds)
        } else {
            root(self)
        }
    }
    pub fn tag(mut self) -> Self {
        self.tag = true;
        self
    }
}
impl<C: Children> Value for CreateTree<C> {
    type Type = Object<Tree>;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.query(
            path,
            |tree| {
                unsafe extern "C" fn child<C: Children>(context: *const (), index: usize) -> u32 {
                    let children = unsafe { &*context.cast::<C>() };
                    code(children.child(index).and_then(|child| child.build_at(None)))
                }
                status(unsafe {
                    fix_sdk_tree(
                        tree.children.len(),
                        tree.tag as i32,
                        child::<C>,
                        (&tree.children as *const C).cast(),
                    )
                })
            },
            |child, path| child.build_at(path),
        )
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        self.query(
            path,
            |_| Ok(ValueType::Object),
            |child, path| child.value_type_at(path),
        )
    }
    fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
        self.query(
            path,
            |_| Ok(DataType::Tree),
            |child, path| child.data_type_at(path),
        )
    }
    fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
        self.query(
            path,
            |_| Err(Error::WrongType),
            |child, path| child.encode_type_at(path),
        )
    }
    fn len_at(&self, path: Option<&Path<'_>>) -> Result<usize, Error> {
        self.query(
            path,
            |tree| Ok(tree.children.len()),
            |child, path| child.len_at(path),
        )
    }
    fn is_tag_at(&self, path: Option<&Path<'_>>) -> Result<bool, Error> {
        self.query(
            path,
            |tree| Ok(tree.tag),
            |child, path| child.is_tag_at(path),
        )
    }
    fn read_at(
        &self,
        path: Option<&Path<'_>>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        self.query(
            path,
            |_| Err(Error::WrongType),
            |child, path| child.read_at(path, offset, bytes),
        )
    }
}

macro_rules! tuples {
    (@types) => { Nil };
    (@types $first:ident $(, $rest:ident)*) => { Cons<$first, tuples!(@types $($rest),*)> };
    ($($t:ident: $index:tt),+) => {
        impl<$($t: Value),+> From<($($t,)+)>
            for CreateTree<tuples!(@types $($t),+)>
        {
            fn from(value: ($($t,)+)) -> Self {
                crate::tree![$(value.$index),+]
            }
        }
        impl<R: Lookup + Clone, $($t: FixType),+> TryFrom<Handle<Object<Tree>, R>>
            for ($(Handle<$t, Child<R>>,)+)
        {
            type Error = Error;
            fn try_from(value: Handle<Object<Tree>, R>) -> Result<Self, Error> {
                let length = [$(stringify!($t)),+].len();
                if value.len()? != length {
                    return Err(Error::WrongType);
                }
                Ok(($(Handle::<Any, _>::new(Child {
                    parent: value.location.clone(),
                    index: $index,
                }).checked::<$t>()?,)+))
            }
        }
        impl<'a, R: Lookup, $($t: FixType),+> TryFrom<&'a Handle<Object<Tree>, R>>
            for ($(Handle<$t, Child<&'a R>>,)+)
        {
            type Error = Error;
            fn try_from(value: &'a Handle<Object<Tree>, R>) -> Result<Self, Error> {
                Handle::<Object<Tree>, _>::new(&value.location).try_into()
            }
        }

        impl<Storage: Children + Clone, $($t: FixType),+> TryFrom<CreateTree<Storage>>
            for ($(Cast<$t, Select<CreateTree<Storage>>>,)+)
        {
            type Error = Error;
            fn try_from(value: CreateTree<Storage>) -> Result<Self, Error> {
                if value.len()? != [$(stringify!($t)),+].len() { return Err(Error::WrongType); }
                Ok(($(checked(Select { parent: value.clone(), index: $index })?,)+))
            }
        }
        impl<'a, $($t: FixType),+> TryFrom<&'a dyn Value<Type = Object<Tree>>>
            for ($(Cast<$t, Select<&'a dyn Value<Type = Object<Tree>>>>,)+)
        {
            type Error = Error;
            fn try_from(value: &'a dyn Value<Type = Object<Tree>>) -> Result<Self, Error> {
                if value.len()? != [$(stringify!($t)),+].len() { return Err(Error::WrongType); }
                Ok(($(checked(Select { parent: value, index: $index })?,)+))
            }
        }
    };
}
impl From<()> for CreateTree<Nil> {
    fn from(_: ()) -> Self {
        crate::tree![]
    }
}
impl<R: Lookup> TryFrom<Handle<Object<Tree>, R>> for () {
    type Error = Error;
    fn try_from(value: Handle<Object<Tree>, R>) -> Result<Self, Error> {
        if value.is_empty()? {
            Ok(())
        } else {
            Err(Error::WrongType)
        }
    }
}
impl<R: Lookup> TryFrom<&Handle<Object<Tree>, R>> for () {
    type Error = Error;
    fn try_from(value: &Handle<Object<Tree>, R>) -> Result<Self, Error> {
        Handle::<Object<Tree>, _>::new(&value.location).try_into()
    }
}
tuples!(A: 0);
tuples!(A: 0, B: 1);
tuples!(A: 0, B: 1, C: 2);
tuples!(A: 0, B: 1, C: 2, D: 3);
tuples!(A: 0, B: 1, C: 2, D: 3, E: 4);
tuples!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
tuples!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6);
tuples!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7);
tuples!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8);
tuples!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9);
tuples!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10);
tuples!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10, L: 11);
impl<C: Children> CreateTree<C> {
    pub fn iter(&self) -> impl Iterator<Item = Select<&Self>> + Clone {
        (0..self.children.len()).map(move |index| Select {
            parent: self,
            index,
        })
    }
    pub fn children(&self) -> Vec<Select<&Self>> {
        self.iter().collect()
    }
}
impl Tree {
    pub fn create<V: Value, C: AsRef<[V]>>(children: C) -> CreateTree<Slice<V, C>> {
        Self::__create(Slice(children, PhantomData), false)
    }
    #[allow(clippy::should_implement_trait)]
    pub fn from_iter<I: IntoIterator>(children: I) -> CreateTree<Iter<I::IntoIter>>
    where
        I::IntoIter: Clone,
        I::Item: Value,
    {
        Self::__create(Iter(children.into_iter()), false)
    }
    pub fn tag<I: IntoIterator>(children: I) -> CreateTree<Iter<I::IntoIter>>
    where
        I::IntoIter: Clone,
        I::Item: Value,
    {
        Self::from_iter(children).tag()
    }
    #[doc(hidden)]
    pub fn __create<C: Children>(children: C, tag: bool) -> CreateTree<C> {
        CreateTree { children, tag }
    }
}
#[macro_export]
macro_rules! tree {
    (@tuple) => { $crate::__Nil };
    (@tuple $first:expr $(, $rest:expr)* $(,)?) => { $crate::__Cons($first, $crate::tree!(@tuple $($rest),*)) };
    ($($child:expr),* $(,)?) => { $crate::Tree::__create($crate::tree!(@tuple $($child),*), false) };
}

#[derive(Clone, Copy)]
pub struct Select<V> {
    pub(super) parent: V,
    pub(super) index: usize,
}
impl<V: Value> Value for Select<V> {
    type Type = Any;
    delegate_value!(
        this,
        path,
        &this.parent,
        Some(&Path {
            index: this.index,
            next: path
        })
    );
}
