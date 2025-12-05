extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use libm::round;
use rand::{RngCore, SeedableRng, rngs::SmallRng, seq::SliceRandom};

#[derive(Debug, Clone)]
pub struct UnboundedInputHostGenerator {
    localhost: String,
    remotehost: String,
    ratio_local: f64,
    rng: SmallRng,
}

impl UnboundedInputHostGenerator {
    pub fn new(localhost: String, remotehost: String, ratio_local: f64) -> Self {
        UnboundedInputHostGenerator {
            localhost,
            remotehost,
            ratio_local,
            rng: SmallRng::seed_from_u64(42),
        }
    }

    pub fn next(&mut self) -> Option<String> {
        let sample: f64 = self.rng.next_u64() as f64;

        if sample / (u64::MAX as f64) < self.ratio_local {
            Some(self.localhost.clone())
        } else {
            Some(self.remotehost.clone())
        }
    }
}

// Generates exactly `total_runs` inputs
// with the exact specified ratio of local to remote hosts
#[derive(Debug, Clone)]
pub struct BoundedInputHostGenerator {
    localhost: String,
    remotehost: String,
    index: usize,
    data: Vec<bool>,
}

impl BoundedInputHostGenerator {
    pub fn new(localhost: String, remotehost: String, total_runs: usize, ratio_local: f64) -> Self {
        let mut rng = SmallRng::seed_from_u64(42);
        let local_count = round(ratio_local * total_runs as f64) as usize;
        let remote_count = total_runs - local_count;

        let mut data = alloc::vec![true; local_count];
        data.extend(alloc::vec![false; remote_count]);
        data.shuffle(&mut rng);

        BoundedInputHostGenerator {
            localhost,
            remotehost,
            index: 0,
            data,
        }
    }

    pub fn next(&mut self) -> Option<String> {
        if self.index >= self.data.len() {
            return None;
        }

        let host = if self.data[self.index] {
            self.localhost.clone()
        } else {
            self.remotehost.clone()
        };

        self.index += 1;
        Some(host)
    }
}
