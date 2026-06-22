extern crate alloc;
use alloc::boxed::Box;
use alloc::vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Failed to parse message
    ParserError,
    /// Failed to serialize message
    SerializerError,
}

pub trait FixedMsg: Sized {
    const SIZE: usize;

    fn encode(&self, out: &mut [u8]) -> Result<(), Error>;
    fn decode(input: &[u8]) -> Result<Self, Error>;
}

pub trait IntoBytes {
    fn into_boxed_slice(self) -> Box<[u8]>;
}

impl<T: FixedMsg> IntoBytes for T {
    fn into_boxed_slice(self: T) -> Box<[u8]> {
        let mut buf = vec![0; T::SIZE];
        self.encode(&mut buf).unwrap();
        buf.into_boxed_slice()
    }
}

pub trait TagRepresentation: Sized + Copy {
    const TAG_SIZE: usize;

    fn write_le(self, out: &mut [u8]) -> Result<(), Error>;
    fn extract_le(input: &[u8]) -> Result<(Self, &[u8]), Error>;
}

macro_rules! impl_tag_representation {
    ($($type:ty), +) => { $(
            impl TagRepresentation for $type {
                const TAG_SIZE: usize = size_of::<$type>();

                fn write_le(self, out: &mut [u8]) -> Result<(), Error> {
                    out.get_mut(..Self::TAG_SIZE)
                        .ok_or(Error::SerializerError)?
                        .copy_from_slice(&self.to_le_bytes());
                    Ok(())
                }

                fn extract_le(input: &[u8]) -> Result<(Self, &[u8]), Error> {
                    let bytes = input.get(..Self::TAG_SIZE).ok_or(Error::ParserError)?;
                    let value = <$type>::from_le_bytes(bytes.try_into().map_err(|_| Error::ParserError)?);
                    Ok((value, &input[Self::TAG_SIZE..]))
                }
            }
        )+
    };
}

impl_tag_representation!(u8, i32, u64, i64, usize);

pub trait Tag: Sized {
    type Representation: TagRepresentation;
    const SIZE: usize = <Self::Representation>::TAG_SIZE;

    fn to_representation(&self) -> Self::Representation;
    fn from_representation(representation: Self::Representation) -> Result<Self, Error>;

    fn write_tag(&self, out: &mut [u8]) -> Result<(), Error> {
        self.to_representation().write_le(out)
    }

    fn extract_tag(input: &[u8]) -> Result<(Self, &[u8]), Error> {
        let (tag, rest) = Self::Representation::extract_le(input)?;
        Ok((Self::from_representation(tag)?, rest))
    }
}

#[macro_export]
macro_rules! impl_tag_enum {
    (
        #[repr($repr:ty)]
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $variant:ident = $value:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr($repr)]
        $vis enum $name {
            $( $variant = $value ),+
        }

        impl Tag for $name {
            type Representation = $repr;

            fn to_representation(&self) -> Self::Representation {
                *self as Self::Representation
            }

            fn from_representation( representation: Self::Representation ) -> Result<Self, Error> {
                match representation {
                    $( $value => Ok(Self::$variant), )+
                    _ => Err( Error::ParserError ),
                }
            }
        }
    };
}

pub trait VariableMsg: Sized {
    fn encoded_len(&self) -> usize;
    fn encode(&self, out: &mut [u8]) -> Result<(), Error>;
    fn decode(input: &[u8]) -> Result<Self, Error>;

    fn to_boxed_slice(&self) -> Result<Box<[u8]>, Error> {
        let mut buf = vec![0u8; self.encoded_len()];
        self.encode(&mut buf)?;
        Ok(buf.into_boxed_slice())
    }
}

pub type FieldTagRepresentation = usize;

pub fn write_field(out: &mut [u8], bytes: &[u8]) -> Result<usize, Error> {
    bytes.len().write_le(out)?;
    let chunk_length = FieldTagRepresentation::TAG_SIZE + bytes.len();
    out.get_mut(FieldTagRepresentation::TAG_SIZE..chunk_length)
        .ok_or(Error::SerializerError)?
        .copy_from_slice(bytes);
    Ok(chunk_length)
}

pub fn extract_field(input: &[u8]) -> Result<(&[u8], &[u8]), Error> {
    let (field_length, rest) = FieldTagRepresentation::extract_le(input)?;
    let field = rest.get(..field_length).ok_or(Error::ParserError)?;
    Ok((field, &rest[field_length..]))
}

// Error catching
pub fn ensure_empty(rest: &[u8]) -> Result<(), Error> {
    if rest.is_empty() {
        Ok(())
    } else {
        Err(Error::ParserError)
    }
}
