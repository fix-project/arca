use crate::{paging::AugmentedPageTable, prelude::*};

type GetPage<T> = <<T as HardwarePageTable>::Entry as HardwarePageTableEntry>::Page;
type GetTable<T> = <<T as HardwarePageTable>::Entry as HardwarePageTableEntry>::Table;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageTable {
    PageTable2MB(Page<AugmentedPageTable<PageTable2MB>>),
    PageTable1GB(Page<AugmentedPageTable<PageTable1GB>>),
    PageTable512GB(Page<AugmentedPageTable<PageTable512GB>>),
    PageTable256TB(Page<AugmentedPageTable<PageTable256TB>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry<T: HardwarePageTable> {
    UniquePage(UniquePage<GetPage<T>>),
    SharedPage(SharedPage<GetPage<T>>),
    UniqueTable(UniquePage<AugmentedPageTable<GetTable<T>>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnyEntry {
    Entry4KB(Entry<PageTable2MB>),
    Entry2MB(Entry<PageTable1GB>),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AddressSpace {
    #[default]
    AddressSpace0B,
    AddressSpace4KB(usize, Entry<PageTable2MB>),
    AddressSpace2MB(usize, Entry<PageTable1GB>),
    AddressSpace1GB(usize, Entry<PageTable512GB>),
}

trait Embiggen: Sized + HardwarePageTable {
    fn from_unique_page(
        index: usize,
        page: UniquePage<GetPage<Self>>,
    ) -> UniquePage<AugmentedPageTable<Self>> {
        let mut pt = AugmentedPageTable::<Self>::new();
        pt.entry_mut(index).map_unique(page);
        pt
    }

    fn from_shared_page(
        index: usize,
        page: SharedPage<GetPage<Self>>,
    ) -> UniquePage<AugmentedPageTable<Self>> {
        let mut pt = AugmentedPageTable::<Self>::new();
        pt.entry_mut(index).map_shared(page);
        pt
    }

    fn from_unique_table(
        index: usize,
        table: UniquePage<AugmentedPageTable<GetTable<Self>>>,
    ) -> UniquePage<AugmentedPageTable<Self>> {
        let mut pt = AugmentedPageTable::<Self>::new();
        pt.entry_mut(index).chain_unique(table);
        pt
    }

    fn from_entry(index: usize, entry: Entry<Self>) -> UniquePage<AugmentedPageTable<Self>> {
        match entry {
            Entry::UniquePage(p) => Self::from_unique_page(index, p),
            Entry::SharedPage(p) => Self::from_shared_page(index, p),
            Entry::UniqueTable(t) => Self::from_unique_table(index, t),
        }
    }
}

impl<T: HardwarePageTable> Embiggen for T {}

impl AddressSpace {
    pub fn new() -> AddressSpace {
        Default::default()
    }

    pub fn map(&mut self, address: usize, entry: AnyEntry) {
        let mut this = AddressSpace::AddressSpace0B;
        core::mem::swap(&mut this, self);
        *self = match (this, entry) {
            (AddressSpace::AddressSpace0B, AnyEntry::Entry4KB(x)) => {
                AddressSpace::AddressSpace4KB(address >> 12, x)
            }
            (AddressSpace::AddressSpace0B, AnyEntry::Entry2MB(x)) => {
                AddressSpace::AddressSpace2MB(address >> 21, x)
            }
            (AddressSpace::AddressSpace4KB(offset, entry), AnyEntry::Entry4KB(x)) => {
                let page_num = address >> 12;
                if offset == page_num {
                    AddressSpace::AddressSpace4KB(offset, x)
                } else {
                    let mut this = AddressSpace::AddressSpace4KB(offset, entry);
                    this.embiggen();
                    this.map(address, AnyEntry::Entry4KB(x));
                    this
                }
            }
            (AddressSpace::AddressSpace4KB(offset, entry), AnyEntry::Entry2MB(x)) => {
                let mut this = AddressSpace::AddressSpace4KB(offset, entry);
                this.embiggen();
                this.map(address, AnyEntry::Entry2MB(x));
                this
            }
            (AddressSpace::AddressSpace2MB(offset, mut entry), AnyEntry::Entry4KB(x)) => {
                let page_num = address >> 21;
                if page_num == offset {
                    let index = (address >> 12) & 0x1ff;
                    match &mut entry {
                        Entry::UniquePage(_) => todo!(),
                        Entry::SharedPage(_) => todo!(),
                        Entry::UniqueTable(t1) => match x {
                            Entry::UniquePage(p2) => {
                                t1.entry_mut(index).map_unique(p2);
                            }
                            Entry::SharedPage(p2) => {
                                t1.entry_mut(index).map_shared(p2);
                            }
                            Entry::UniqueTable(t2) => {
                                t1.entry_mut(index).chain_unique(t2);
                            }
                        },
                    };
                    AddressSpace::AddressSpace2MB(offset, entry)
                } else {
                    let mut this = AddressSpace::AddressSpace2MB(offset, entry);
                    this.embiggen();
                    this.map(address, AnyEntry::Entry4KB(x));
                    this
                }
            }
            (AddressSpace::AddressSpace2MB(offset, entry), AnyEntry::Entry2MB(x)) => {
                let page_num = address >> 21;
                if offset == page_num {
                    AddressSpace::AddressSpace2MB(offset, x)
                } else {
                    let mut this = AddressSpace::AddressSpace2MB(offset, entry);
                    this.embiggen();
                    this.map(address, AnyEntry::Entry2MB(x));
                    this
                }
            }
            (AddressSpace::AddressSpace1GB(offset, entry), AnyEntry::Entry4KB(x)) => {
                let index = (address >> 12) & 0x1ff;
                let x = Entry::UniqueTable(PageTable2MB::from_entry(index, x));
                let mut this = AddressSpace::AddressSpace1GB(offset, entry);
                this.map(address, AnyEntry::Entry2MB(x));
                this
            }
            (AddressSpace::AddressSpace1GB(offset, mut entry), AnyEntry::Entry2MB(x)) => {
                let page_num = address >> 30;
                if page_num == offset {
                    let index = (address >> 21) & 0x1ff;
                    match &mut entry {
                        Entry::UniquePage(_) => todo!(),
                        Entry::SharedPage(_) => todo!(),
                        Entry::UniqueTable(t1) => match x {
                            Entry::UniquePage(p2) => {
                                t1.entry_mut(index).map_unique(p2);
                            }
                            Entry::SharedPage(p2) => {
                                t1.entry_mut(index).map_shared(p2);
                            }
                            Entry::UniqueTable(t2) => {
                                t1.entry_mut(index).chain_unique(t2);
                            }
                        },
                    };
                    AddressSpace::AddressSpace1GB(offset, entry)
                } else {
                    let mut this = AddressSpace::AddressSpace1GB(offset, entry);
                    this.embiggen();
                    this.map(address, AnyEntry::Entry2MB(x));
                    this
                }
            }
        };
    }

    pub fn embiggen(&mut self) {
        let mut this = AddressSpace::new();
        core::mem::swap(&mut this, self);
        match this {
            AddressSpace::AddressSpace0B => unreachable!(),
            AddressSpace::AddressSpace4KB(offset, entry) => {
                let inner = offset & 0x1ff;
                let outer = offset >> 9;
                this = AddressSpace::AddressSpace2MB(
                    outer,
                    Entry::UniqueTable(PageTable2MB::from_entry(inner, entry)),
                );
            }
            AddressSpace::AddressSpace2MB(offset, entry) => {
                let inner = offset & 0x1ff;
                let outer = offset >> 9;
                this = AddressSpace::AddressSpace1GB(
                    outer,
                    Entry::UniqueTable(PageTable1GB::from_entry(inner, entry)),
                );
            }
            AddressSpace::AddressSpace1GB(offset, _entry) => {
                todo!("growing address space 1gb @ {offset}");
            }
        }
        core::mem::swap(&mut this, self);
    }
}
