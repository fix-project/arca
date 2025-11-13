use serde::de::{VariantAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Serialize};

use core::fmt;

use crate::Runtime;
use crate::prelude::*;

impl<R: Runtime> Serialize for Value<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Null(null) => serializer.serialize_newtype_variant("Value", 0, "Null", null),
            Value::Word(word) => serializer.serialize_newtype_variant("Value", 1, "Word", word),
            Value::Blob(blob) => serializer.serialize_newtype_variant("Value", 2, "Blob", blob),
            Value::Tuple(tuple) => serializer.serialize_newtype_variant("Value", 3, "Tuple", tuple),
            Value::Page(page) => serializer.serialize_newtype_variant("Value", 4, "Page", page),
            Value::Table(table) => serializer.serialize_newtype_variant("Value", 5, "Table", table),
            Value::Function(function) => {
                serializer.serialize_newtype_variant("Value", 6, "Function", function)
            }
        }
    }
}

impl<R: Runtime> Serialize for Entry<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Entry::Null(_) => serializer.serialize_unit_variant("Entry", 0, "Null"),
            Entry::ROPage(page) => serializer.serialize_newtype_variant("Entry", 1, "ROPage", page),
            Entry::RWPage(page) => serializer.serialize_newtype_variant("Entry", 2, "RWPage", page),
            Entry::ROTable(table) => {
                serializer.serialize_newtype_variant("Entry", 3, "ROTable", table)
            }
            Entry::RWTable(table) => {
                serializer.serialize_newtype_variant("Entry", 4, "RWTable", table)
            }
        }
    }
}

impl<R: Runtime> Serialize for Null<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ().serialize(serializer)
    }
}

impl<R: Runtime> Serialize for Word<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.read().serialize(serializer)
    }
}

impl<R: Runtime> Serialize for Blob<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.with_ref(|v| serializer.serialize_bytes(v))
    }
}

impl<R: Runtime> Serialize for Tuple<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.with_ref(|x| {
            let mut s = serializer.serialize_seq(Some(x.len()))?;
            for value in x.iter() {
                s.serialize_element(&value)?
            }
            s.end()
        })
    }
}

impl<R: Runtime> Serialize for Page<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.with_ref(|v| serializer.serialize_bytes(v))
    }
}

impl<R: Runtime> Serialize for Table<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.with_ref(|x| {
            let len = x.iter().filter(|x| !x.is_null()).count();
            let mut s = serializer.serialize_map(Some(len + 1))?;
            s.serialize_entry("len", &self.len())?;
            for (i, value) in x.iter().enumerate() {
                if !value.is_null() {
                    s.serialize_entry(&i, &value)?
                }
            }
            s.end()
        })
    }
}

impl<R: Runtime> Serialize for Function<R> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let data = self.read_cloned();
        data.serialize(serializer)
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Value<R> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_enum(
            "Value",
            &["Null", "Word", "Blob", "Tuple", "Page", "Table", "Function"],
            ValueVisitor::<R>(core::marker::PhantomData),
        )
    }
}
struct ValueVisitor<R: Runtime>(core::marker::PhantomData<R>);

impl<'de, R: Runtime> Visitor<'de> for ValueVisitor<R> {
    type Value = Value<R>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("arca value enum")
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::EnumAccess<'de>,
    {
        let (variant, variant_access) = data.variant()?;
        match variant {
            0 => {
                let v = variant_access.newtype_variant::<Null<R>>()?;
                Ok(Value::Null(v))
            }
            1 => {
                let v = variant_access.newtype_variant::<Word<R>>()?;
                Ok(Value::Word(v))
            }
            2 => {
                let v = variant_access.newtype_variant::<Blob<R>>()?;
                Ok(Value::Blob(v))
            }
            3 => {
                let v = variant_access.newtype_variant::<Tuple<R>>()?;
                Ok(Value::Tuple(v))
            }
            4 => {
                let v = variant_access.newtype_variant::<Page<R>>()?;
                Ok(Value::Page(v))
            }
            5 => {
                let v = variant_access.newtype_variant::<Table<R>>()?;
                Ok(Value::Table(v))
            }
            6 => {
                let v = variant_access.newtype_variant::<Function<R>>()?;
                Ok(Value::Function(v))
            }
            unknown_id => Err(serde::de::Error::unknown_variant(
                "unknown", // TODO: be more descriptive here
                &["Null", "Word", "Blob", "Tuple", "Page", "Table"],
            )),
        }
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Null<R> {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Null::new())
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Word<R> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_u64(WordVisitor::<R>(core::marker::PhantomData))
    }
}

struct WordVisitor<R>(core::marker::PhantomData<R>);

impl<'de, R: Runtime> Visitor<'de> for WordVisitor<R> {
    type Value = Word<R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an arca word")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Word::new(v))
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Blob<R> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(BlobVisitor::<R>(core::marker::PhantomData))
    }
}

struct BlobVisitor<R>(core::marker::PhantomData<R>);

impl<'de, R: Runtime> Visitor<'de> for BlobVisitor<R> {
    type Value = Blob<R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a Vec<u8>")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Blob::new(v))
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Tuple<R> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(TupleVisitor::<R>(core::marker::PhantomData))
    }
}

struct TupleVisitor<R>(core::marker::PhantomData<R>);

impl<'de, R: Runtime> Visitor<'de> for TupleVisitor<R> {
    type Value = Tuple<R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a seq")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut items = Vec::<Value<R>>::new();
        while let Some(elem) = seq.next_element()? {
            items.push(elem);
        }

        let mut tuple = Tuple::new(items.len());
        for (idx, value) in items.into_iter().enumerate() {
            tuple.set(idx, value);
        }
        Ok(tuple)
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Page<R> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(PageVisitor::<R>(core::marker::PhantomData))
    }
}

struct PageVisitor<R>(core::marker::PhantomData<R>);

impl<'de, R: Runtime> Visitor<'de> for PageVisitor<R> {
    type Value = Page<R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut page: Page<R> = Page::new(v.len());
        page.write(0, v);
        Ok(page)
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Table<R> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(TableVisitor::<R>(core::marker::PhantomData))
    }
}

struct TableVisitor<R>(core::marker::PhantomData<R>);

impl<'de, R: Runtime> Visitor<'de> for TableVisitor<R> {
    type Value = Table<R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let (first_key, first_value): (alloc::string::String, usize) =
            map.next_entry()?.expect("at least one element needed");
        assert_eq!(first_key, "len");
        let mut table = Table::new(first_value);
        while let Some((key, value)) = map.next_entry()? {
            table.set(key, value).expect("map entry");
        }
        Ok(table)
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Entry<R> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_enum(
            "Entry",
            &["Null", "ROPage", "RWPage", "ROTable", "RWTable"],
            EntryVisitor::<R>(core::marker::PhantomData),
        )
    }
}

struct EntryVisitor<R>(core::marker::PhantomData<R>);

impl<'de, R: Runtime> Visitor<'de> for EntryVisitor<R> {
    type Value = Entry<R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an enum")
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::EnumAccess<'de>,
    {
        let (variant, variant_access) = data.variant()?;
        match variant {
            0 => Ok(Entry::Null(0)),
            1 => {
                let v = variant_access.newtype_variant::<Page<R>>()?;
                Ok(Entry::ROPage(v))
            }
            2 => {
                let v = variant_access.newtype_variant::<Page<R>>()?;
                Ok(Entry::RWPage(v))
            }
            3 => {
                let v = variant_access.newtype_variant::<Table<R>>()?;
                Ok(Entry::ROTable(v))
            }
            4 => {
                let v = variant_access.newtype_variant::<Table<R>>()?;
                Ok(Entry::RWTable(v))
            }
            other => Err(serde::de::Error::unknown_variant(
                "other",
                &["Null", "ROPage", "RWPage", "ROTable", "RWTable"],
            )),
        }
    }
}

impl<'de, R: Runtime> Deserialize<'de> for Function<R> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = Value::deserialize(deserializer)?;
        Ok(Function::new(val).unwrap())
    }
}
