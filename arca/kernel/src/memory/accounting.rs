#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub unused: usize,
    pub used: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accounting {
    reserved: usize,
    pub unreserved: Usage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unavailable {
    pub required: usize,
    pub available: usize,
}

impl Usage {
    pub(super) fn use_memory(&mut self, bytes: usize) -> Result<(), Unavailable> {
        if bytes > self.unused {
            return Err(Unavailable {
                required: bytes,
                available: self.unused,
            });
        }
        self.unused -= bytes;
        self.used += bytes;
        Ok(())
    }

    pub(super) fn release(&mut self, bytes: usize) {
        self.used = self
            .used
            .checked_sub(bytes)
            .expect("unbalanced memory release");
        self.unused += bytes;
    }
}

impl Accounting {
    pub fn new(total: usize, unreserved_used: usize) -> Self {
        Self {
            reserved: 0,
            unreserved: Usage {
                unused: total
                    .checked_sub(unreserved_used)
                    .expect("memory usage exceeds capacity"),
                used: unreserved_used,
            },
        }
    }

    pub fn reserve(&mut self, bytes: usize) -> Result<(), Unavailable> {
        if bytes > self.unreserved.unused {
            return Err(Unavailable {
                required: bytes,
                available: self.unreserved.unused,
            });
        }
        self.unreserved.unused -= bytes;
        self.reserved += bytes;
        Ok(())
    }

    pub fn use_unreserved(&mut self, bytes: usize) -> Result<(), Unavailable> {
        self.unreserved.use_memory(bytes)
    }

    pub fn release_unreserved(&mut self, bytes: usize) {
        self.unreserved.release(bytes);
    }

    pub fn unreserve(&mut self, reservation: Usage) {
        self.remove_reservation(reservation.unused + reservation.used);
        self.unreserved.unused += reservation.unused;
        self.unreserved.used += reservation.used;
    }

    pub fn unreserve_used(&mut self, reservation: &mut Usage, bytes: usize) {
        reservation.used = reservation
            .used
            .checked_sub(bytes)
            .expect("unbalanced memory transfer");
        self.remove_reservation(bytes);
        self.unreserved.used += bytes;
    }

    fn remove_reservation(&mut self, bytes: usize) {
        self.reserved = self
            .reserved
            .checked_sub(bytes)
            .expect("unbalanced reservation");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_reservations_exclude_kernel_and_scheduler_usage() {
        let mut accounting = Accounting::new(1024, 128);
        accounting.reserve(512).unwrap();
        accounting.reserve(256).unwrap();
        let mut first = Usage {
            unused: 512,
            used: 0,
        };
        let mut second = Usage {
            unused: 256,
            used: 0,
        };
        accounting.use_unreserved(128).unwrap();
        assert_eq!(accounting.unreserved.unused, 0);
        assert!(accounting.use_unreserved(1).is_err());
        first.use_memory(384).unwrap();
        second.use_memory(192).unwrap();
        assert!(second.use_memory(65).is_err());
        first.release(128);
        assert!(accounting.reserve(1).is_err());
        accounting.release_unreserved(64);
        accounting.reserve(64).unwrap();
        accounting.unreserve(first);
        assert_eq!(
            accounting.unreserved,
            Usage {
                unused: 256,
                used: 448
            }
        );
        accounting.release_unreserved(256);
        assert_eq!(
            accounting.unreserved,
            Usage {
                unused: 512,
                used: 192
            }
        );
        assert_eq!(accounting.reserved, second.unused + second.used + 64);
    }

    #[test]
    fn retained_objects_stay_used_after_their_invocation_ends() {
        let mut accounting = Accounting::new(1024, 128);
        accounting.reserve(896).unwrap();
        let mut invocation = Usage {
            unused: 896,
            used: 0,
        };
        invocation.use_memory(768).unwrap();
        accounting.unreserve_used(&mut invocation, 512);
        assert_eq!(accounting.unreserved.unused, 0);
        assert_eq!(accounting.unreserved.used, 640);
        invocation.release(256);
        accounting.unreserve(invocation);
        assert_eq!(accounting.reserved, 0);
        assert_eq!(accounting.unreserved.unused, 384);
        assert!(accounting.reserve(385).is_err());
        accounting.release_unreserved(512);
        accounting.reserve(896).unwrap();
        assert_eq!(accounting.reserved, 896);
        assert_eq!(accounting.unreserved.used, 128);
        assert_eq!(accounting.unreserved.unused, 0);
    }
}
