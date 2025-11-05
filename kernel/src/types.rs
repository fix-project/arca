mod arca;
mod blob;
mod function;
mod null;
pub(crate) mod page;
mod runtime;
pub(crate) mod table;
mod tuple;
mod value;
mod word;

pub mod internal {
    pub use super::arca::LoadedArca;
    pub use super::blob::Blob;
    pub use super::function::Function;
    pub use super::null::Null;
    pub use super::page::Page;
    pub use super::table::Table;
    pub use super::tuple::Tuple;
    pub use super::value::Value;
    pub use super::value::ValueRef;
    pub use super::word::Word;
}

pub use arca::Arca;
pub use runtime::Error;
pub use runtime::Runtime;

pub type Null = ::arca::Null<Runtime>;
pub type Word = ::arca::Word<Runtime>;
pub type Blob = ::arca::Blob<Runtime>;
pub type Tuple = ::arca::Tuple<Runtime>;
pub type Page = ::arca::Page<Runtime>;
pub type Table = ::arca::Table<Runtime>;
pub type Function = ::arca::Function<Runtime>;
pub type Value = ::arca::Value<Runtime>;
pub type ValueRef<'a> = ::arca::ValueRef<'a, Runtime>;
pub type Entry = ::arca::Entry<Runtime>;
