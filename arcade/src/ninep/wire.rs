use core::iter::Extend;
use core::{fmt::Display, str::Utf8Error};
use kernel::prelude::*;
use serde::{
    Deserializer as _, Serialize,
    de::{self, IntoDeserializer, SeqAccess},
    ser,
};

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Error {
    Message(String),
    IncompatibleType(&'static str),
    TooLong,
    InvalidVariantName(&'static str),
    UnexpectedEndOfData,
    TrailingBytes,
    NotSelfDescribing,
    InvalidString(Utf8Error),
}

pub fn to_bytes(value: impl ser::Serialize) -> Result<Vec<u8>> {
    let mut serializer = Serializer { output: Vec::new() };
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

pub fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_bytes(s);
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.input.is_empty() {
        Ok(t)
    } else {
        Err(Error::TrailingBytes)
    }
}

pub fn to_bytes_with_len(value: impl ser::Serialize) -> Result<Vec<u8>> {
    let mut serializer = Serializer {
        output: vec![0, 0, 0, 0],
    };
    value.serialize(&mut serializer)?;
    let n = serializer.output.len();
    serializer.output[0..4].copy_from_slice(&(n as u32).to_le_bytes());
    Ok(serializer.output)
}

pub fn from_bytes_with_len<'a, T>(s: &'a [u8]) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    if s.len() < 4 {
        return Err(Error::UnexpectedEndOfData);
    }
    let mut deserializer = Deserializer::from_bytes(s[4..].try_into().unwrap());
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.input.is_empty() {
        Ok(t)
    } else {
        Err(Error::TrailingBytes)
    }
}

impl ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Error::Message(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Error::Message(msg.to_string())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        todo!()
    }
}

pub struct Serializer {
    output: Vec<u8>,
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();

    type Error = Error;

    type SerializeSeq = Self;

    type SerializeTuple = Self;

    type SerializeTupleStruct = Self;

    type SerializeTupleVariant = Self;

    type SerializeMap = Self;

    type SerializeStruct = Self;

    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend((v as u8).to_le_bytes());
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> core::result::Result<Self::Ok, Self::Error> {
        self.output.extend(v.to_le_bytes());
        Ok(())
    }

    fn serialize_char(self, v: char) -> core::result::Result<Self::Ok, Self::Error> {
        Err(Error::IncompatibleType("char"))
    }

    fn serialize_str(self, v: &str) -> core::result::Result<Self::Ok, Self::Error> {
        let n: u16 = v.len().try_into().map_err(|_| Error::TooLong)?;
        n.serialize(&mut *self);
        self.output.extend(v.as_bytes());
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> core::result::Result<Self::Ok, Self::Error> {
        let n: u16 = v.len().try_into().map_err(|_| Error::TooLong)?;
        n.serialize(&mut *self);
        self.output.extend(v);
        Ok(())
    }

    fn serialize_none(self) -> core::result::Result<Self::Ok, Self::Error> {
        Err(Error::IncompatibleType("None"))
    }

    fn serialize_some<T>(self, value: &T) -> core::result::Result<Self::Ok, Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        Err(Error::IncompatibleType("Some"))
    }

    fn serialize_unit(self) -> core::result::Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> core::result::Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> core::result::Result<Self::Ok, Self::Error> {
        let name: u8 = str::parse(variant).map_err(|e| Error::InvalidVariantName(variant))?;
        name.serialize(&mut *self)?;
        Ok(())
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> core::result::Result<Self::Ok, Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(self)?;
        Ok(())
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> core::result::Result<Self::Ok, Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        let name: u8 = str::parse(variant).map_err(|e| Error::InvalidVariantName(variant))?;
        name.serialize(&mut *self)?;
        value.serialize(&mut *self)?;
        Ok(())
    }

    fn serialize_seq(
        self,
        len: Option<usize>,
    ) -> core::result::Result<Self::SerializeSeq, Self::Error> {
        let len = len.ok_or(Error::IncompatibleType("unsized Seq"))?;
        let len: u16 = len.try_into().map_err(|_| Error::TooLong)?;
        len.serialize(&mut *self)?;
        Ok(self)
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> core::result::Result<Self::SerializeTuple, Self::Error> {
        Ok(self)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> core::result::Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> core::result::Result<Self::SerializeTupleVariant, Self::Error> {
        let name: u8 = str::parse(variant).map_err(|e| Error::InvalidVariantName(variant))?;
        name.serialize(&mut *self)?;
        Ok(self)
    }

    fn serialize_map(
        self,
        len: Option<usize>,
    ) -> core::result::Result<Self::SerializeMap, Self::Error> {
        Err(Error::IncompatibleType("map"))
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> core::result::Result<Self::SerializeStruct, Self::Error> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> core::result::Result<Self::SerializeStructVariant, Self::Error> {
        let name: u8 = str::parse(variant).map_err(|e| Error::InvalidVariantName(variant))?;
        name.serialize(&mut *self)?;
        Ok(self)
    }
}

impl<'a> ser::SerializeSeq for &'a mut Serializer {
    type Ok = ();

    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> core::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> core::result::Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = ();

    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> core::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> core::result::Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> core::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> core::result::Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> core::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> core::result::Result<Self::Ok, Self::Error> {
        todo!()
    }
}
impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = ();

    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> core::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn serialize_value<T>(&mut self, value: &T) -> core::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> core::result::Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeStruct for &'a mut Serializer {
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> core::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)?;
        Ok(())
    }

    fn end(self) -> core::result::Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> core::result::Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)?;
        Ok(())
    }

    fn end(self) -> core::result::Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

pub struct Deserializer<'de> {
    input: &'de [u8],
}

impl<'de> Deserializer<'de> {
    pub fn from_bytes(input: &'de [u8]) -> Self {
        Deserializer { input }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::NotSelfDescribing)
    }

    fn deserialize_bool<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let head = self.input.get(0).ok_or(Error::UnexpectedEndOfData)?;
        self.input = &self.input[1..];
        visitor.visit_bool(*head != 0)
    }

    fn deserialize_i8<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i16<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i32<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i64<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_u8<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let head = self.input.get(0).ok_or(Error::UnexpectedEndOfData)?;
        self.input = &self.input[1..];
        visitor.visit_u8(*head)
    }

    fn deserialize_u16<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let (head, rest) = self
            .input
            .split_at_checked(2)
            .ok_or(Error::UnexpectedEndOfData)?;
        self.input = rest;
        visitor.visit_u16(u16::from_le_bytes(head.try_into().unwrap()))
    }

    fn deserialize_u32<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let (head, rest) = self
            .input
            .split_at_checked(4)
            .ok_or(Error::UnexpectedEndOfData)?;
        self.input = rest;
        visitor.visit_u32(u32::from_le_bytes(head.try_into().unwrap()))
    }

    fn deserialize_u64<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let (head, rest) = self
            .input
            .split_at_checked(8)
            .ok_or(Error::UnexpectedEndOfData)?;
        self.input = rest;
        visitor.visit_u64(u64::from_le_bytes(head.try_into().unwrap()))
    }

    fn deserialize_f32<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_f64<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_char<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_str<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let (head, rest) = self
            .input
            .split_at_checked(2)
            .ok_or(Error::UnexpectedEndOfData)?;
        let size = u16::from_le_bytes(head.try_into().unwrap()) as usize;
        let (s, rest) = rest
            .split_at_checked(size)
            .ok_or(Error::UnexpectedEndOfData)?;
        self.input = rest;
        let s = core::str::from_utf8(s).map_err(Error::InvalidString)?;
        visitor.visit_str(s)
    }

    fn deserialize_string<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let (head, rest) = self
            .input
            .split_at_checked(2)
            .ok_or(Error::UnexpectedEndOfData)?;
        let size = u16::from_le_bytes(head.try_into().unwrap()) as usize;
        let (s, rest) = rest
            .split_at_checked(size)
            .ok_or(Error::UnexpectedEndOfData)?;
        self.input = rest;
        let s = core::str::from_utf8(s).map_err(Error::InvalidString)?;
        visitor.visit_string(s.to_owned())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let (head, rest) = self
            .input
            .split_at_checked(2)
            .ok_or(Error::UnexpectedEndOfData)?;
        let size = u16::from_le_bytes(head.try_into().unwrap()) as usize;
        let (s, rest) = rest
            .split_at_checked(size)
            .ok_or(Error::UnexpectedEndOfData)?;
        self.input = rest;
        visitor.visit_bytes(s)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let (head, rest) = self
            .input
            .split_at_checked(2)
            .ok_or(Error::UnexpectedEndOfData)?;
        let size = u16::from_le_bytes(head.try_into().unwrap()) as usize;
        let (s, rest) = rest
            .split_at_checked(size)
            .ok_or(Error::UnexpectedEndOfData)?;
        self.input = rest;
        visitor.visit_byte_buf(s.to_vec())
    }

    fn deserialize_option<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_unit<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_tuple<V>(
        self,
        len: usize,
        visitor: V,
    ) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        struct Access<'a, 'de> {
            deserializer: &'a mut Deserializer<'de>,
            len: usize,
        };

        impl<'a, 'de> de::SeqAccess<'de> for Access<'a, 'de> {
            type Error = Error;

            fn next_element_seed<T>(
                &mut self,
                seed: T,
            ) -> core::result::Result<Option<T::Value>, Self::Error>
            where
                T: de::DeserializeSeed<'de>,
            {
                if self.len == 0 {
                    return Ok(None);
                } else {
                    self.len -= 1;
                    let value = seed.deserialize(&mut *self.deserializer)?;
                    Ok(Some(value))
                }
            }
        }

        visitor.visit_seq(Access {
            deserializer: self,
            len,
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_map<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_tuple(fields.len(), visitor)
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_enum(self)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_u8(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::NotSelfDescribing)
    }
}

impl<'de, 'a> de::EnumAccess<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    type Variant = Self;

    fn variant_seed<V>(
        self,
        seed: V,
    ) -> core::result::Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let head = self.input.get(0).ok_or(Error::UnexpectedEndOfData)?;
        self.input = &self.input[1..];
        let name = head.to_string();
        let val = seed.deserialize(name.into_deserializer())?;
        Ok((val, self))
    }
}

impl<'de, 'a> de::VariantAccess<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> core::result::Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> core::result::Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> core::result::Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_tuple(fields.len(), visitor)
    }
}
