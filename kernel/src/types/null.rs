use arca::DataType;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Null;

impl Null {
    pub fn new() -> Self {
        Null
    }
}

impl Default for Null {
    fn default() -> Self {
        Self::new()
    }
}

impl arca::RuntimeType for Null {
    type Runtime = super::Runtime;
}

impl arca::ValueType for Null {
    const DATATYPE: DataType = DataType::Null;
}

impl arca::Null for Null {}
