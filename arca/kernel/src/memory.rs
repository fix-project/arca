use crate::kthread::KCondvar;
use alloc::sync::Arc;
use common::{buddy::BuddyAllocator, util::spinlock::SpinLock};
use serde::{Deserialize, Serialize};

pub mod accounting;

struct Manager {
    accounting: Option<accounting::Accounting>,
    reservations: usize,
}

impl Manager {
    fn accounting(&mut self) -> &mut accounting::Accounting {
        self.accounting.get_or_insert_with(|| {
            accounting::Accounting::new(BuddyAllocator.total_size(), BuddyAllocator.used_size())
        })
    }
}

static MANAGER: SpinLock<Manager> = SpinLock::new(Manager {
    accounting: None,
    reservations: 0,
});
static CHANGED: KCondvar = KCondvar::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionError {
    pub required: usize,
    pub available: usize,
}

pub struct Admission {
    reserved: bool,
    usage: SpinLock<accounting::Usage>,
}

impl core::fmt::Debug for Admission {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Admission")
            .field("reserved", &self.reserved)
            .finish_non_exhaustive()
    }
}

impl From<accounting::Unavailable> for AdmissionError {
    fn from(error: accounting::Unavailable) -> Self {
        Self {
            required: error.required,
            available: error.available,
        }
    }
}

pub fn available() -> usize {
    MANAGER.lock().accounting().unreserved.unused
}

pub fn wait_until_available(required: usize) -> Result<(), AdmissionError> {
    let mut manager = MANAGER.lock();
    loop {
        let available = manager.accounting().unreserved.unused;
        if required <= available {
            return Ok(());
        }
        if manager.reservations == 0 || required > BuddyAllocator.total_size() {
            return Err(AdmissionError {
                required,
                available,
            });
        }
        manager = CHANGED.wait_spin(&MANAGER, manager);
    }
}

impl Admission {
    pub fn new(required: usize) -> Result<Self, AdmissionError> {
        let mut manager = MANAGER.lock();
        manager
            .accounting()
            .reserve(required)
            .map_err(AdmissionError::from)?;
        manager.reservations += 1;
        Ok(Self {
            reserved: true,
            usage: SpinLock::new(accounting::Usage {
                unused: required,
                used: 0,
            }),
        })
    }

    pub fn unreserved() -> Self {
        Self {
            reserved: false,
            usage: SpinLock::new(accounting::Usage::default()),
        }
    }

    fn consume(&self, bytes: usize) -> Result<(), AdmissionError> {
        let mut manager = MANAGER.lock();
        let mut usage = self.usage.lock();
        if self.reserved {
            manager
                .accounting()
                .use_reserved(&mut usage, bytes)
                .map_err(Into::into)
        } else {
            manager
                .accounting()
                .use_unreserved(bytes)
                .map_err(AdmissionError::from)?;
            usage.used += bytes;
            Ok(())
        }
    }

    fn release(&self, bytes: usize) {
        let mut manager = MANAGER.lock();
        let mut usage = self.usage.lock();
        if self.reserved {
            manager.accounting().release_reserved(&mut usage, bytes);
        } else {
            usage.used -= bytes;
            manager.accounting().release_unreserved(bytes);
        }
        CHANGED.notify_all();
    }

    pub fn retain(&self, bytes: usize) -> FreeMemory {
        let mut manager = MANAGER.lock();
        let mut usage = self.usage.lock();
        if self.reserved {
            manager.accounting().unreserve_used(&mut usage, bytes);
        } else {
            usage.used = usage
                .used
                .checked_sub(bytes)
                .expect("unbalanced retained memory");
        }
        FreeMemory(bytes)
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        let mut manager = MANAGER.lock();
        let usage = *self.usage.lock();
        if self.reserved {
            let mut remaining = usage;
            manager
                .accounting()
                .release_reserved(&mut remaining, usage.used);
            manager.accounting().unreserve(remaining);
            manager.reservations -= 1;
        } else {
            manager.accounting().release_unreserved(usage.used);
        }
        CHANGED.notify_all();
    }
}

#[derive(Debug)]
pub struct FreeMemory(usize);

impl FreeMemory {
    pub fn new(bytes: usize) -> Result<Self, AdmissionError> {
        MANAGER
            .lock()
            .accounting()
            .use_unreserved(bytes)
            .map_err(AdmissionError::from)?;
        Ok(Self(bytes))
    }

    pub fn split(&mut self, bytes: usize) -> Self {
        self.0 = self.0.checked_sub(bytes).expect("memory credit exceeded");
        Self(bytes)
    }
}

impl Drop for FreeMemory {
    fn drop(&mut self) {
        MANAGER.lock().accounting().release_unreserved(self.0);
        CHANGED.notify_all();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exhausted;

#[derive(Debug, Serialize, Deserialize)]
pub struct Budget {
    enforced: bool,
    limit: usize,
    used: usize,
    #[serde(skip)]
    account: Option<Arc<Admission>>,
    #[serde(skip)]
    capacity_failure: Option<AdmissionError>,
}

impl Clone for Budget {
    fn clone(&self) -> Self {
        Self {
            enforced: self.enforced,
            limit: self.limit,
            used: self.used,
            account: None,
            capacity_failure: None,
        }
    }
}

impl Budget {
    pub fn new(limit: usize) -> Self {
        Self {
            enforced: true,
            limit,
            used: 0,
            account: None,
            capacity_failure: None,
        }
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
    pub fn enforce(&mut self, enforced: bool) {
        self.enforced = enforced;
    }
    pub fn attach(&mut self, account: Arc<Admission>) -> Result<(), Exhausted> {
        assert!(self.account.is_none());
        if let Err(error) = account.consume(self.used) {
            self.capacity_failure = Some(error);
            return Err(Exhausted);
        }
        self.account = Some(account);
        Ok(())
    }

    pub fn capacity_failure(&self) -> Option<AdmissionError> {
        self.capacity_failure
    }

    pub fn retain(&self, bytes: usize) -> Option<FreeMemory> {
        self.account.as_ref().map(|account| account.retain(bytes))
    }
    pub fn used(&self) -> usize {
        self.used
    }

    pub fn charge(&mut self, bytes: usize) -> Result<(), Exhausted> {
        let used = self.used.checked_add(bytes).ok_or(Exhausted)?;
        if self.enforced && used > self.limit {
            log::trace!(
                "memory quota exceeded: used={} requested={bytes} limit={}",
                self.used,
                self.limit
            );
            return Err(Exhausted);
        }
        if let Some(account) = &self.account
            && let Err(error) = account.consume(bytes)
        {
            self.capacity_failure = Some(error);
            return Err(Exhausted);
        }
        self.used = used;
        Ok(())
    }

    pub fn release(&mut self, bytes: usize) {
        self.used -= bytes;
        if let Some(account) = &self.account {
            account.release(bytes);
        }
    }

    pub fn set_limit(&mut self, limit: usize) -> Result<(), Exhausted> {
        if self.enforced && self.used > limit {
            return Err(Exhausted);
        }
        self.limit = limit;
        Ok(())
    }
}
