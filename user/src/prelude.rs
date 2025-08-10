use super::Runtime;
use arca::Runtime as _;

pub use arca::DataType;

pub type Blob = arca::Blob<Runtime>;
pub type Entry = arca::Entry<Runtime>;
pub type Function = arca::Function<Runtime>;
pub type Null = arca::Null<Runtime>;
pub type Page = arca::Page<Runtime>;
pub type Table = arca::Table<Runtime>;
pub type Tuple = arca::Tuple<Runtime>;
pub type Value = arca::Value<Runtime>;
pub type ValueRef<'a> = arca::ValueRef<'a, Runtime>;
pub type Word = arca::Word<Runtime>;
pub use arca::Continuation;

pub use crate::os;
