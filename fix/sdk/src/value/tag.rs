use super::*;

#[derive(Clone, Copy)]
struct Author<'a> {
    focus: &'a Focus,
    depth: usize,
}
impl Lookup for Author<'_> {
    fn load(&self, mut path: Option<&Path<'_>>) -> Result<(), Error> {
        self.focus.load(None)?;
        for _ in 0..self.depth {
            status(unsafe { fix_sdk_entry(1) })?;
        }
        while let Some(entry) = path {
            status(unsafe { fix_sdk_entry(entry.index) })?;
            path = entry.next;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct CreateTag<'a, C> {
    fields: CreateTree<C>,
    author: Handle<Object<Any>, Author<'a>>,
    additional: usize,
    append: bool,
}
impl<C: Children> CreateTree<C> {
    pub fn tag<'a>(
        self,
        combination: &'a Handle<Object<Tree>, Focus>,
        depth: usize,
    ) -> Result<CreateTag<'a, C>, Error> {
        let author = Handle::new(Author {
            focus: &combination.location,
            depth,
        });
        author.value_type()?;
        let authors = Cast::<Object<Tree>, _>::try_from(self.get(0))?;
        let additional = usize::try_from(authors.len()?)?;
        let append = !(0..additional).any(|index| equal(&authors.get(index), &author));
        Ok(CreateTag {
            fields: self,
            author,
            additional,
            append,
        })
    }
}
impl<C: Children> CreateTag<'_, C> {
    fn view(&self) -> Result<CreateTree<impl Children + '_>, Error> {
        let extra = Cast::<Object<Tree>, _>::try_from(self.fields.get(0))?;
        let original = extra.erase();
        let author = self.author;
        let length = self.additional;
        let authors = Tree::from_iter((0..length + usize::from(self.append)).map(move |index| {
            choose(
                index < length,
                Select {
                    parent: extra,
                    index,
                },
                author.erase(),
            )
        }));
        let authors = choose(self.append, authors.erase(), original);
        Ok(Tree::from_iter((0..self.fields.children.len()).map(
            move |index| choose(index == 0, authors.clone(), self.fields.get(index)),
        )))
    }
}
impl<C: Children> Value for CreateTag<'_, C> {
    type Type = Object<Tree>;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        if path.is_none() {
            self.fields.build(Some(self.author.location.depth))
        } else {
            self.view()?.build_at(path)
        }
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        self.view()?.value_type_at(path)
    }
    fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
        self.view()?.data_type_at(path)
    }
    fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
        self.view()?.encode_type_at(path)
    }
    fn len_at(&self, path: Option<&Path<'_>>) -> Result<u64, Error> {
        self.view()?.len_at(path)
    }
    fn is_tag_at(&self, path: Option<&Path<'_>>) -> Result<bool, Error> {
        if path.is_none() {
            Ok(true)
        } else {
            self.view()?.is_tag_at(path)
        }
    }
    fn read_at(
        &self,
        path: Option<&Path<'_>>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        self.view()?.read_at(path, offset, bytes)
    }
}
impl<C: Children, V: Value> PartialEq<V> for CreateTag<'_, C> {
    fn eq(&self, other: &V) -> bool {
        equal(self, other)
    }
}
