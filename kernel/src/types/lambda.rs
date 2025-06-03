use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lambda {
    pub arca: Arca,
    pub idx: usize,
}

impl arca::RuntimeType for Lambda {
    type Runtime = Runtime;
}

impl arca::ValueType for Lambda {
    const DATATYPE: DataType = DataType::Lambda;
}

impl arca::Lambda for Lambda {
    fn apply(self, argument: arca::associated::Value<Self>) -> arca::associated::Thunk<Self> {
        let mut arca = self.arca;
        let idx = self.idx;
        arca.descriptors_mut()[idx] = argument;
        Thunk::new(arca)
    }

    fn read(self) -> (arca::associated::Thunk<Self>, usize) {
        todo!()
    }
}
