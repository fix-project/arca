use common::message::Handle;

use crate::paging::Impossible;
use crate::prelude::*;

use crate::page::CowPage;

type TableImpossible = AugmentedPageTable<Impossible>;
type Table2MB = AugmentedPageTable<PageTable2MB>;
type Table1GB = AugmentedPageTable<PageTable1GB>;
type Table512GB = AugmentedPageTable<PageTable512GB>;
// type Table256TB = AugmentedPageTable<PageTable256TB>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Table {
    Table2MB(CowPage<Table2MB>),
    Table1GB(CowPage<Table1GB>),
    Table512GB(CowPage<Table512GB>),
}

impl Table {
    pub fn new(size: usize) -> Table {
        if size <= Table2MB::SIZE {
            Table::Table2MB(Default::default())
        } else if size <= Table1GB::SIZE {
            Table::Table1GB(Default::default())
        } else if size <= Table512GB::SIZE {
            Table::Table512GB(Default::default())
        } else {
            panic!();
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new(0)
    }
}

impl arca::RuntimeType for Table {
    type Runtime = Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl arca::ValueType for Table {
    const DATATYPE: DataType = DataType::Table;
}

impl arca::Table for Table {
    fn take(&mut self, index: usize) -> arca::Entry<Self> {
        match self {
            Table::Table2MB(table) => table.entry_mut(index).unmap().into(),
            Table::Table1GB(table) => table.entry_mut(index).unmap().into(),
            Table::Table512GB(table) => table.entry_mut(index).unmap().into(),
        }
    }

    fn put(
        &mut self,
        index: usize,
        entry: arca::Entry<Self>,
    ) -> Result<arca::Entry<Self>, arca::Entry<Self>> {
        Ok(match self {
            Table::Table2MB(table) => table.entry_mut(index).replace(entry.try_into()?).into(),
            Table::Table1GB(table) => table.entry_mut(index).replace(entry.try_into()?).into(),
            Table::Table512GB(table) => table.entry_mut(index).replace(entry.try_into()?).into(),
        })
    }

    fn size(&self) -> usize {
        match self {
            Table::Table2MB(_) => 1 << 21,
            Table::Table1GB(_) => 1 << 30,
            Table::Table512GB(_) => 1 << 39,
        }
    }
}

impl<P: HardwarePage, T: HardwarePageTable> From<AugmentedUnmappedPage<P, T>> for arca::Entry<Table>
where
    Page: From<CowPage<P>>,
    Table: From<CowPage<AugmentedPageTable<T>>>,
{
    fn from(value: AugmentedUnmappedPage<P, T>) -> Self {
        match value {
            AugmentedUnmappedPage::None => arca::Entry::Null(Null::new()),
            AugmentedUnmappedPage::UniquePage(page) => {
                arca::Entry::RWPage(CowPage::Unique(page).into())
            }
            AugmentedUnmappedPage::SharedPage(page) => {
                arca::Entry::ROPage(CowPage::Shared(page).into())
            }
            AugmentedUnmappedPage::Global(_) => todo!(),
            AugmentedUnmappedPage::UniqueTable(page) => {
                arca::Entry::RWTable(CowPage::Unique(page).into())
            }
            AugmentedUnmappedPage::SharedTable(page) => {
                arca::Entry::RWTable(CowPage::Shared(page).into())
            }
        }
    }
}

impl<P: HardwarePage, T: HardwarePageTable> TryFrom<arca::Entry<Table>>
    for AugmentedUnmappedPage<P, T>
where
    CowPage<P>: TryFrom<Page, Error = Page>,
    CowPage<AugmentedPageTable<T>>: TryFrom<Table, Error = Table>,
{
    type Error = arca::Entry<Table>;

    fn try_from(value: arca::Entry<Table>) -> Result<Self, Self::Error> {
        Ok(match value {
            arca::Entry::Null(_) => AugmentedUnmappedPage::None,
            arca::Entry::ROPage(page) => AugmentedUnmappedPage::SharedPage({
                let page: CowPage<P> = page.try_into().map_err(arca::Entry::ROPage)?;
                page.shared()
            }),
            arca::Entry::RWPage(page) => AugmentedUnmappedPage::UniquePage({
                let page: CowPage<P> = page.try_into().map_err(arca::Entry::RWPage)?;
                page.unique()
            }),
            arca::Entry::ROTable(table) => AugmentedUnmappedPage::SharedTable({
                let table: CowPage<AugmentedPageTable<T>> =
                    table.try_into().map_err(arca::Entry::ROTable)?;
                table.shared()
            }),
            arca::Entry::RWTable(table) => AugmentedUnmappedPage::UniqueTable({
                let table: CowPage<AugmentedPageTable<T>> =
                    table.try_into().map_err(arca::Entry::RWTable)?;
                table.unique()
            }),
        })
    }
}

impl From<CowPage<TableImpossible>> for Table {
    fn from(_: CowPage<TableImpossible>) -> Self {
        unreachable!()
    }
}

impl From<CowPage<Table2MB>> for Table {
    fn from(value: CowPage<Table2MB>) -> Self {
        Table::Table2MB(value)
    }
}

impl From<CowPage<Table1GB>> for Table {
    fn from(value: CowPage<Table1GB>) -> Self {
        Table::Table1GB(value)
    }
}

impl From<CowPage<Table512GB>> for Table {
    fn from(value: CowPage<Table512GB>) -> Self {
        Table::Table512GB(value)
    }
}

impl TryFrom<Table> for CowPage<TableImpossible> {
    type Error = Table;

    fn try_from(_: Table) -> Result<Self, Self::Error> {
        unreachable!();
    }
}

impl TryFrom<Table> for CowPage<Table2MB> {
    type Error = Table;

    fn try_from(value: Table) -> Result<Self, Self::Error> {
        match value {
            Table::Table2MB(page) => Ok(page),
            _ => Err(value),
        }
    }
}

impl TryFrom<Table> for CowPage<Table1GB> {
    type Error = Table;

    fn try_from(value: Table) -> Result<Self, Self::Error> {
        match value {
            Table::Table1GB(page) => Ok(page),
            _ => Err(value),
        }
    }
}

impl TryFrom<Table> for CowPage<Table512GB> {
    type Error = Table;

    fn try_from(value: Table) -> Result<Self, Self::Error> {
        match value {
            Table::Table512GB(page) => Ok(page),
            _ => Err(value),
        }
    }
}

impl TryFrom<Handle> for Table {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let (ptr, size) = value.read();
            let unique = ptr & 1 == 1;
            let ptr = ptr & !1;
            unsafe {
                Ok(match size {
                    val if val == 1 << 21 => {
                        Table::Table2MB(CowPage::from_raw(unique, ptr as *mut _))
                    }
                    val if val == 1 << 30 => {
                        Table::Table1GB(CowPage::from_raw(unique, ptr as *mut _))
                    }
                    val if val == 1 << 39 => {
                        Table::Table512GB(CowPage::from_raw(unique, ptr as *mut _))
                    }
                    _ => unreachable!(),
                })
            }
        } else {
            Err(value)
        }
    }
}

impl From<Table> for Handle {
    fn from(value: Table) -> Self {
        let size = value.size();
        let (unique, mut ptr) = match value {
            Table::Table2MB(page) => {
                let (unique, ptr) = CowPage::into_raw(page);
                (unique, ptr as usize)
            }
            Table::Table1GB(page) => {
                let (unique, ptr) = CowPage::into_raw(page);
                (unique, ptr as usize)
            }
            Table::Table512GB(page) => {
                let (unique, ptr) = CowPage::into_raw(page);
                (unique, ptr as usize)
            }
        };
        if unique {
            ptr |= 1;
        }
        Handle::new(DataType::Table, (ptr, size))
    }
}
