use crate::paging::Impossible;
use crate::prelude::*;

use crate::types::page::Page;

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

pub type Entry = arca::Entry<Runtime>;

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

impl Table {
    pub fn set(&mut self, index: usize, entry: Entry) -> Result<Entry, Entry> {
        Ok(match self {
            Table::Table2MB(table) => table.entry_mut(index).replace(entry.try_into()?).into(),
            Table::Table1GB(table) => table.entry_mut(index).replace(entry.try_into()?).into(),
            Table::Table512GB(table) => table.entry_mut(index).replace(entry.try_into()?).into(),
        })
    }

    pub fn get(&self, index: usize) -> Entry {
        match self {
            Table::Table2MB(table) => table
                .entry(index)
                .map(|x| x.clone().unmap().into())
                .unwrap_or_else(|| arca::Entry::Null(1 << 12)),
            Table::Table1GB(table) => table
                .entry(index)
                .map(|x| x.clone().unmap().into())
                .unwrap_or_else(|| arca::Entry::Null(1 << 21)),
            Table::Table512GB(table) => table
                .entry(index)
                .map(|x| x.clone().unmap().into())
                .unwrap_or_else(|| arca::Entry::Null(1 << 30)),
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Table::Table2MB(_) => 1 << 21,
            Table::Table1GB(_) => 1 << 30,
            Table::Table512GB(_) => 1 << 39,
        }
    }
}

impl<P: HardwarePage, T: HardwarePageTable> From<AugmentedUnmappedPage<P, T>> for Entry
where
    Page: From<CowPage<P>>,
    Table: From<CowPage<AugmentedPageTable<T>>>,
{
    fn from(value: AugmentedUnmappedPage<P, T>) -> Self {
        match value {
            AugmentedUnmappedPage::None => arca::Entry::Null(core::cmp::max(P::SIZE, T::SIZE)),
            AugmentedUnmappedPage::UniquePage(page) => {
                arca::Entry::RWPage(arca::Page::from_inner(CowPage::Unique(page).into()))
            }
            AugmentedUnmappedPage::SharedPage(page) => {
                arca::Entry::ROPage(arca::Page::from_inner(CowPage::Shared(page).into()))
            }
            AugmentedUnmappedPage::Global(_) => todo!(),
            AugmentedUnmappedPage::UniqueTable(page) => {
                arca::Entry::RWTable(arca::Table::from_inner(CowPage::Unique(page).into()))
            }
            AugmentedUnmappedPage::SharedTable(page) => {
                arca::Entry::RWTable(arca::Table::from_inner(CowPage::Shared(page).into()))
            }
        }
    }
}

impl<P: HardwarePage, T: HardwarePageTable> TryFrom<Entry> for AugmentedUnmappedPage<P, T>
where
    CowPage<P>: TryFrom<Page, Error = Page>,
    CowPage<AugmentedPageTable<T>>: TryFrom<Table, Error = Table>,
{
    type Error = Entry;

    fn try_from(value: Entry) -> Result<Self, Self::Error> {
        Ok(match value {
            arca::Entry::Null(_) => AugmentedUnmappedPage::None,
            arca::Entry::ROPage(page) => AugmentedUnmappedPage::SharedPage({
                let page: CowPage<P> = page
                    .into_inner()
                    .try_into()
                    .map_err(|x| arca::Entry::ROPage(arca::Page::from_inner(x)))?;
                page.shared()
            }),
            arca::Entry::RWPage(page) => AugmentedUnmappedPage::UniquePage({
                let page: CowPage<P> = page
                    .into_inner()
                    .try_into()
                    .map_err(|x| arca::Entry::RWPage(arca::Page::from_inner(x)))?;
                page.unique()
            }),
            arca::Entry::ROTable(table) => AugmentedUnmappedPage::SharedTable({
                let table: CowPage<AugmentedPageTable<T>> = table
                    .into_inner()
                    .try_into()
                    .map_err(|x| arca::Entry::ROTable(arca::Table::from_inner(x)))?;
                table.shared()
            }),
            arca::Entry::RWTable(table) => AugmentedUnmappedPage::UniqueTable({
                let table: CowPage<AugmentedPageTable<T>> = table
                    .into_inner()
                    .try_into()
                    .map_err(|x| arca::Entry::RWTable(arca::Table::from_inner(x)))?;
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
