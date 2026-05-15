use crate::{
    fixruntime::{CouponHelper, DeterministicEquivRuntime, Operator, RuntimeError},
    memoryruntime::MemoryRuntime,
    vmcommon::CouponTrades,
};
use fixhandle::rawhandle::FixHandle;

pub struct MockRuntime {
    memory_runtime: MemoryRuntime,
}

impl Default for MockRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl MockRuntime {
    pub fn new() -> Self {
        Self {
            memory_runtime: MemoryRuntime::new(),
        }
    }
}

impl DeterministicEquivRuntime for MockRuntime {
    type BlobData<'a> = &'a [u8];
    type TreeData<'a> = &'a [u8];
    type Handle = FixHandle;
    type Error = RuntimeError;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle {
        self.memory_runtime.create_blob_i32(data)
    }

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        self.memory_runtime.create_blob_i64(data)
    }

    fn create_blob(&mut self, data: &[u8]) -> Self::Handle {
        self.memory_runtime.create_blob(data)
    }

    fn create_tree(&mut self, data: &[u8]) -> Self::Handle {
        self.memory_runtime.create_tree(data)
    }

    fn get_blob<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::BlobData<'a>, Self::Error> {
        self.memory_runtime.get_blob(handle)
    }

    fn get_tree<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::TreeData<'a>, Self::Error> {
        self.memory_runtime.get_tree(handle)
    }
}

impl CouponHelper for MockRuntime {}

impl Operator for MockRuntime {
    fn trade(
        &mut self,
        _trade_type: CouponTrades,
        _coupons: FixHandle,
        _lhs: FixHandle,
        _rhs: FixHandle,
    ) -> FixHandle {
        todo!()
    }

    fn eval(&mut self, _handle: FixHandle) -> FixHandle {
        todo!()
    }

    fn apply(&mut self, handle: FixHandle) -> FixHandle {
        let span = self.get_tree(&handle).unwrap();
        if span.len() < 3 {
            panic!()
            // return Err(RuntimeError::OOB);
        }

        let tree_entry = Self::get_tree_entry(span, 0);
        let function = self.get_blob(&tree_entry).unwrap();
        if function != b"+" {
            panic!()
            // return Err(RuntimeError::UnexpectedFunction);
        }

        let left_bytes: [u8; 8] = self
            .get_blob(&Self::get_tree_entry(span, 1))
            .unwrap()
            .try_into()
            .map_err(|_| RuntimeError::OOB)
            .unwrap();
        let left = u64::from_le_bytes(left_bytes);
        let right_bytes: [u8; 8] = self
            .get_blob(&Self::get_tree_entry(span, 2))
            .unwrap()
            .try_into()
            .map_err(|_| RuntimeError::OOB)
            .unwrap();
        let right = u64::from_le_bytes(right_bytes);

        self.create_blob_i64(left + right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::bitpack::BitPack;
    #[test]
    fn test_add() {
        let mut mock_runtime = MockRuntime::new();

        let one_literal = mock_runtime.create_blob_i64(1);
        let two_literal = mock_runtime.create_blob_i64(2);
        let plus_literal = mock_runtime.create_blob(b"+");

        let mut tree_bytes = Vec::new();
        tree_bytes.extend_from_slice(&plus_literal.pack());
        tree_bytes.extend_from_slice(&one_literal.pack());
        tree_bytes.extend_from_slice(&two_literal.pack());
        let tree = mock_runtime.create_tree(&tree_bytes);
        let result = mock_runtime.apply(tree);

        assert_eq!(
            mock_runtime.get_blob(&result).expect("valid result blob"),
            3_i64.to_le_bytes()
        );
    }
}
