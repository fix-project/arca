use crate::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageTable {
    PageTable2MB(Page<PageTable2MB>),
    PageTable1GB(Page<PageTable1GB>),
    PageTable512GB(Page<PageTable512GB>),
    PageTable256TB(Page<PageTable256TB>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UniqueEntry<T: HardwarePageTable> {
    Page(UniquePage<<T::Entry as HardwarePageTableEntry>::Page>),
    Table(UniquePage<<T::Entry as HardwarePageTableEntry>::Table>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnyUniqueEntry {
    Entry4KB(UniqueEntry<PageTable2MB>),
    // Entry2MB(UniqueEntry<PageTable1GB>),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AddressSpace {
    #[default]
    AddressSpace0B,
    AddressSpace4KB(usize, UniqueEntry<PageTable2MB>),
    AddressSpace2MB(usize, UniqueEntry<PageTable1GB>),
}

impl AddressSpace {
    pub fn new() -> AddressSpace {
        Default::default()
    }

    pub fn map_unique(&mut self, address: usize, entry: AnyUniqueEntry) {
        let l0 = address & 0xfff;
        let l1 = address & 0x1ff000;
        let l2 = address & 0x3fe00000;
        let l3 = address & 0x7fc0000000;
        let l4 = address & 0xff8000000000;

        match entry {
            AnyUniqueEntry::Entry4KB(entry) => {
                assert_eq!(l0, 0);
                match self {
                    AddressSpace::AddressSpace0B => {
                        *self = AddressSpace::AddressSpace4KB(l4 | l3 | l2 | l1, entry)
                    }
                    AddressSpace::AddressSpace4KB(_, _) => {
                        self.embiggen();
                        self.map_unique(address, AnyUniqueEntry::Entry4KB(entry));
                    }
                    AddressSpace::AddressSpace2MB(offset, existing) => {
                        let prefix = *offset & !0x1fffff;
                        assert_eq!(prefix, l4 | l3 | l2);
                        let index = l1 >> 12;
                        match existing {
                            UniqueEntry::Page(_page) => {
                                todo!();
                            }
                            UniqueEntry::Table(table) => {
                                match entry {
                                    UniqueEntry::Page(p) => table[index].map_unique(p),
                                    UniqueEntry::Table(t) => table[index].chain_unique(t),
                                };
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn embiggen(&mut self) {
        let mut this = AddressSpace::new();
        core::mem::swap(&mut this, self);
        match this {
            AddressSpace::AddressSpace0B => unreachable!(),
            AddressSpace::AddressSpace4KB(offset, entry) => {
                let inner = offset & 0x1fffff;
                let index = inner >> 12;
                let outer = offset & !0x1fffff;
                let mut pt = PageTable2MB::new();
                match entry {
                    UniqueEntry::Page(page) => {
                        pt[index].map_unique(page);
                    }
                    UniqueEntry::Table(table) => {
                        pt[index].chain_unique(table);
                    }
                }
                this = AddressSpace::AddressSpace2MB(outer, UniqueEntry::Table(pt));
            }
            AddressSpace::AddressSpace2MB(_offset, _entry) => {
                todo!();
            }
        }
        core::mem::swap(&mut this, self);
    }
}

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct OffsetUniquePage<T: HardwarePage> {
//     offset: usize,
//     page: UniquePage<T>,
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct OffsetUniquePageTable<T: HardwarePageTable> {
//     offset: usize,
//     table: UniquePage<T>,
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum OffsetUniqueEntry<T: HardwarePageTable> {
//     Null(usize),
//     Page(OffsetUniquePage<<T::Entry as HardwarePageTableEntry>::Page>),
//     Table(OffsetUniquePageTable<<T::Entry as HardwarePageTableEntry>::Table>),
// }

// impl<T: HardwarePageTable> OffsetUniqueEntry<T> {
//     pub fn offset(&self) -> usize {
//         match self {
//             OffsetUniqueEntry::Null(offset) => *offset,
//             OffsetUniqueEntry::Page(offset_page) => offset_page.offset,
//             OffsetUniqueEntry::Table(offset_page_table) => offset_page_table.offset,
//         }
//     }

//     pub fn embiggen(self) -> OffsetUniqueEntry<T::Parent> {
//         match self {
//             OffsetUniqueEntry::Null(offset) => {
//                 let inner = offset & (T::SIZE - 1);
//                 let outer = offset & !(T::SIZE - 1);
//                 OffsetUniqueEntry::Null(outer)
//             }
//             OffsetUniqueEntry::Page(offset_unique_page) => {
//                 let offset = offset_unique_page.offset;
//                 let inner = offset & (T::SIZE - 1);
//                 let outer = offset & !(T::SIZE - 1);
//                 let mut pt =
//                     <<T::Parent as HardwarePageTable>::Entry as HardwarePageTableEntry>::Table::new(
//                     );
//                 unsafe {
//                     pt[inner].map_unique(core::mem::transmute(offset_unique_page.page));
//                 }
//                 OffsetUniqueEntry::Table(OffsetUniquePageTable {
//                     offset: outer,
//                     table: pt,
//                 })
//             }
//             OffsetUniqueEntry::Table(offset_unique_page_table) => {
//                 let offset = offset_unique_page_table.offset;
//                 let inner = offset & (T::SIZE - 1);
//                 let outer = offset & !(T::SIZE - 1);
//                 let mut pt =
//                     <<T::Parent as HardwarePageTable>::Entry as HardwarePageTableEntry>::Table::new(
//                     );
//                 unsafe {
//                     pt[inner].chain_unique(core::mem::transmute(offset_unique_page_table.table));
//                 }
//                 OffsetUniqueEntry::Table(OffsetUniquePageTable {
//                     offset: outer,
//                     table: pt,
//                 })
//             }
//         }
//     }
// }

// #[derive(Clone, Debug, Eq, PartialEq)]
// pub enum PageTable {
//     PageTable0B(usize),
//     PageTable4KB(OffsetUniqueEntry<PageTable2MB>),
//     PageTable2MB(OffsetUniqueEntry<PageTable1GB>),
//     PageTable1GB(OffsetUniqueEntry<PageTable512GB>),
//     PageTable512GB(OffsetUniqueEntry<PageTable256TB>),
// }

// impl PageTable {
//     pub fn offset(&self) -> usize {
//         match self {
//             PageTable::PageTable0B(offset) => *offset,
//             PageTable::PageTable4KB(offset_unique_entry) => offset_unique_entry.offset(),
//             PageTable::PageTable2MB(offset_unique_entry) => offset_unique_entry.offset(),
//             PageTable::PageTable1GB(offset_unique_entry) => offset_unique_entry.offset(),
//             PageTable::PageTable512GB(offset_unique_entry) => offset_unique_entry.offset(),
//         }
//     }

//     pub fn embiggen(&mut self) {
//         let mut this = PageTable::PageTable0B(0);
//         core::mem::swap(self, &mut this);
//         match this {
//             PageTable::PageTable0B(offset) => {
//                 let outer = offset & !((1 << 12) - 1);
//                 this = PageTable::PageTable4KB(OffsetUniqueEntry::Null(outer));
//             }
//             PageTable::PageTable4KB(offset_unique_entry) => {
//                 this = PageTable::PageTable2MB(offset_unique_entry.embiggen());
//             }
//             PageTable::PageTable2MB(offset_unique_entry) => {
//                 this = PageTable::PageTable1GB(offset_unique_entry.embiggen());
//             }
//             PageTable::PageTable1GB(offset_unique_entry) => {
//                 this = PageTable::PageTable512GB(offset_unique_entry.embiggen());
//             }
//             PageTable::PageTable512GB(_) => {
//                 panic!("tried to embiggen 512GB page table");
//             }
//         }
//         core::mem::swap(self, &mut this);
//     }

//     pub fn insert_unique(&mut self, child: PageTable) {
//         match (&self, &child) {
//             (PageTable::PageTable0B(_), _) => *self = child,
//             (_, PageTable::PageTable0B(_)) => {}
//             (PageTable::PageTable4KB(_), _) => {
//                 self.embiggen();
//                 self.insert_unique(child);
//             }
//             (PageTable::PageTable2MB(x), PageTable::PageTable4KB(y)) => {
//                 let prefix = self.offset() & !((1 << 21) - 1);
//                 let outer = child.offset() & !((1 << 21) - 1);
//                 let inner = child.offset() & ((1 << 21) - 1);
//                 if prefix == outer {
//                     x.map_unique(inner, y);
//                 } else {
//                     self.embiggen();
//                     child.embiggen();
//                     self.insert_unique(child);
//                 }
//             }
//             (
//                 PageTable::PageTable2MB(offset_unique_entry),
//                 PageTable::PageTable2MB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable2MB(offset_unique_entry),
//                 PageTable::PageTable1GB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable2MB(offset_unique_entry),
//                 PageTable::PageTable512GB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable1GB(offset_unique_entry),
//                 PageTable::PageTable4KB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable1GB(offset_unique_entry),
//                 PageTable::PageTable2MB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable1GB(offset_unique_entry),
//                 PageTable::PageTable1GB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable1GB(offset_unique_entry),
//                 PageTable::PageTable512GB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable512GB(offset_unique_entry),
//                 PageTable::PageTable4KB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable512GB(offset_unique_entry),
//                 PageTable::PageTable2MB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable512GB(offset_unique_entry),
//                 PageTable::PageTable1GB(offset_unique_entry),
//             ) => todo!(),
//             (
//                 PageTable::PageTable512GB(offset_unique_entry),
//                 PageTable::PageTable512GB(offset_unique_entry),
//             ) => todo!(),
//         }
//     }
// }
