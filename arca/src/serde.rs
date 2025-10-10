use serde::Serialize;
use serde::ser::SerializeMap;
use serde::ser::SerializeTuple;

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
            let mut s = serializer.serialize_tuple(self.len())?;
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
            let mut s = serializer.serialize_map(Some(len))?;
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
