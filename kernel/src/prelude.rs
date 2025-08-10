pub(crate) use crate::{
    initcell::{LazyLock, OnceLock},
    vm,
};

pub use alloc::{
    borrow::ToOwned,
    boxed::Box,
    string::{String, ToString},
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};

pub use crate::{
    cpu::{Cpu, Register, RegisterFile, CPU},
    page::{CowPage, Page1GB, Page2MB, Page4KB, SharedPage, UniquePage},
    paging::{
        AugmentedEntry, AugmentedPageTable, AugmentedUnmappedPage, HardwarePage, HardwarePageTable,
        HardwarePageTableEntry, HardwareUnmappedPage, PageTable1GB, PageTable1GBEntry,
        PageTable256TB, PageTable256TBEntry, PageTable2MB, PageTable2MBEntry, PageTable512GB,
        PageTable512GBEntry,
    },
    shutdown,
    types::{Blob, Entry, Function, Null, Page, Runtime, Table, Tuple, Value, Word},
};
pub use arca::DataType;
pub use common::buddy::BuddyAllocator;
pub use common::refcnt::RefCnt;
pub use common::util::channel;
pub use common::util::oneshot;
pub use common::util::rwlock::RwLock;
pub use common::util::sorter;
pub use common::util::spinlock::SpinLock;
pub use macros::kmain;
