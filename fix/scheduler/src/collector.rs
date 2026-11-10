use crate::kernel::{self, Execution};
use crate::shell::{Root, root};
use alloc::vec::Vec;
pub use fixwasm::coupon::{Exchange, Statement};
use fixwasm::{Error, Tree, tree};

#[derive(Clone)]
pub struct Witness {
    pub coupon: Root,
    pub assertion: Root,
}

pub struct Collector {
    callable: Root,
}

impl Collector {
    pub fn new(callable: Root) -> Self {
        Self { callable }
    }

    pub fn exchange(
        &self,
        rule: Exchange,
        lhs: &Root,
        rhs: &Root,
        witnesses: &[Witness],
    ) -> Result<Execution, Error> {
        kernel::invoke(&self.request(rule, lhs, rhs, witnesses)?)
    }

    pub fn request(
        &self,
        rule: Exchange,
        lhs: &Root,
        rhs: &Root,
        witnesses: &[Witness],
    ) -> Result<Root, Error> {
        let witnesses = witnesses
            .iter()
            .map(|witness| root(tree![&witness.coupon, &witness.assertion]))
            .collect::<Result<Vec<_>, Error>>()?;
        root(tree![
            0u64,
            &self.callable,
            rule as u64,
            lhs,
            rhs,
            Tree::from_iter(&witnesses)
        ])
    }
}
