use crate::{
    page::SharedPage,
    paging::{PageTable1GB, PageTable256TB, PageTable2MB, PageTable512GB},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PageTable {
    PageTable2MB(SharedPage<PageTable2MB>),
    PageTable1GB(SharedPage<PageTable1GB>),
    PageTable512GB(SharedPage<PageTable512GB>),
    PageTable256TB(SharedPage<PageTable256TB>),
}
