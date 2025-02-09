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
pub enum UniqueEntry<T: HardwarePageTable> {
    Page(UniquePage<GetPage<T>>),
    Table(UniquePage<AugmentedPageTable<GetTable<T>>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnyUniqueEntry {
    Entry4KB(UniqueEntry<PageTable2MB>),
    Entry2MB(UniqueEntry<PageTable1GB>),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AddressSpace {
    #[default]
    AddressSpace0B,
    AddressSpace4KB(usize, UniqueEntry<PageTable2MB>),
    AddressSpace2MB(usize, UniqueEntry<PageTable1GB>),
    AddressSpace1GB(usize, UniqueEntry<PageTable512GB>),
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

    fn from_unique_table(
        index: usize,
        table: UniquePage<AugmentedPageTable<GetTable<Self>>>,
    ) -> UniquePage<AugmentedPageTable<Self>> {
        let mut pt = AugmentedPageTable::<Self>::new();
        pt.entry_mut(index).chain_unique(table);
        pt
    }

    fn from_unique_entry(
        index: usize,
        entry: UniqueEntry<Self>,
    ) -> UniquePage<AugmentedPageTable<Self>> {
        match entry {
            UniqueEntry::Page(p) => Self::from_unique_page(index, p),
            UniqueEntry::Table(t) => Self::from_unique_table(index, t),
        }
    }
}

impl<T: HardwarePageTable> Embiggen for T {}

impl AddressSpace {
    pub fn new() -> AddressSpace {
        Default::default()
    }

    pub fn map_unique(&mut self, address: usize, entry: AnyUniqueEntry) {
        let mut this = AddressSpace::AddressSpace0B;
        core::mem::swap(&mut this, self);
        *self = match (this, entry) {
            (AddressSpace::AddressSpace0B, AnyUniqueEntry::Entry4KB(x)) => {
                AddressSpace::AddressSpace4KB(address >> 12, x)
            }
            (AddressSpace::AddressSpace0B, AnyUniqueEntry::Entry2MB(x)) => {
                AddressSpace::AddressSpace2MB(address >> 21, x)
            }
            (AddressSpace::AddressSpace4KB(offset, entry), AnyUniqueEntry::Entry4KB(x)) => {
                let page_num = address >> 12;
                if offset == page_num {
                    AddressSpace::AddressSpace4KB(offset, x)
                } else {
                    let mut this = AddressSpace::AddressSpace4KB(offset, entry);
                    this.embiggen();
                    this.map_unique(address, AnyUniqueEntry::Entry4KB(x));
                    this
                }
            }
            (AddressSpace::AddressSpace4KB(offset, entry), AnyUniqueEntry::Entry2MB(x)) => {
                let mut this = AddressSpace::AddressSpace4KB(offset, entry);
                this.embiggen();
                this.map_unique(address, AnyUniqueEntry::Entry2MB(x));
                this
            }
            (AddressSpace::AddressSpace2MB(offset, mut entry), AnyUniqueEntry::Entry4KB(x)) => {
                let page_num = address >> 21;
                if page_num == offset {
                    let index = (address >> 12) & 0x1ff;
                    match &mut entry {
                        UniqueEntry::Page(_) => todo!(),
                        UniqueEntry::Table(t1) => match x {
                            UniqueEntry::Page(p2) => {
                                t1.entry_mut(index).map_unique(p2);
                            }
                            UniqueEntry::Table(t2) => {
                                t1.entry_mut(index).chain_unique(t2);
                            }
                        },
                    };
                    AddressSpace::AddressSpace2MB(offset, entry)
                } else {
                    let mut this = AddressSpace::AddressSpace2MB(offset, entry);
                    this.embiggen();
                    this.map_unique(address, AnyUniqueEntry::Entry4KB(x));
                    this
                }
            }
            (AddressSpace::AddressSpace2MB(offset, entry), AnyUniqueEntry::Entry2MB(x)) => {
                let page_num = address >> 21;
                if offset == page_num {
                    AddressSpace::AddressSpace2MB(offset, x)
                } else {
                    let mut this = AddressSpace::AddressSpace2MB(offset, entry);
                    this.embiggen();
                    this.map_unique(address, AnyUniqueEntry::Entry2MB(x));
                    this
                }
            }
            (AddressSpace::AddressSpace1GB(offset, entry), AnyUniqueEntry::Entry4KB(x)) => {
                let index = (address >> 12) & 0x1ff;
                let x = UniqueEntry::Table(PageTable2MB::from_unique_entry(index, x));
                let mut this = AddressSpace::AddressSpace1GB(offset, entry);
                this.map_unique(address, AnyUniqueEntry::Entry2MB(x));
                this
            }
            (AddressSpace::AddressSpace1GB(offset, mut entry), AnyUniqueEntry::Entry2MB(x)) => {
                let page_num = address >> 30;
                if page_num == offset {
                    let index = (address >> 21) & 0x1ff;
                    match &mut entry {
                        UniqueEntry::Page(_) => todo!(),
                        UniqueEntry::Table(t1) => match x {
                            UniqueEntry::Page(p2) => {
                                t1.entry_mut(index).map_unique(p2);
                            }
                            UniqueEntry::Table(t2) => {
                                t1.entry_mut(index).chain_unique(t2);
                            }
                        },
                    };
                    AddressSpace::AddressSpace1GB(offset, entry)
                } else {
                    let mut this = AddressSpace::AddressSpace1GB(offset, entry);
                    this.embiggen();
                    this.map_unique(address, AnyUniqueEntry::Entry2MB(x));
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
                    UniqueEntry::Table(PageTable2MB::from_unique_entry(inner, entry)),
                );
            }
            AddressSpace::AddressSpace2MB(offset, entry) => {
                let inner = offset & 0x1ff;
                let outer = offset >> 9;
                this = AddressSpace::AddressSpace1GB(
                    outer,
                    UniqueEntry::Table(PageTable1GB::from_unique_entry(inner, entry)),
                );
            }
            AddressSpace::AddressSpace1GB(offset, _entry) => {
                todo!("growing address space 1gb @ {offset}");
            }
        }
        core::mem::swap(&mut this, self);
    }
}
