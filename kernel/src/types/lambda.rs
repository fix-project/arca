use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lambda {
    pub arca: Arca,
    pub idx: usize,
}

impl Lambda {
    pub fn apply<T: Into<Value>>(self, x: T) -> Thunk {
        Thunk::new(self, x)
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedLambda<'_> {
        LoadedLambda {
            arca: self.arca.load(cpu),
            idx: self.idx,
        }
    }
}

#[derive(Debug)]
pub struct LoadedLambda<'a> {
    pub arca: LoadedArca<'a>,
    pub idx: usize,
}

impl<'a> LoadedLambda<'a> {
    pub fn apply<T: Into<Value>>(self, x: T) -> LoadedThunk<'a> {
        LoadedThunk::new(self, x)
    }

    pub fn unload(self) -> Lambda {
        Lambda {
            arca: self.arca.unload(),
            idx: self.idx,
        }
    }
}
