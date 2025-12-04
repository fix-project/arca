extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use libm::round;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;

pub struct InputHostGenerator {
    localhost: String,
    remotehost: String,
    index: usize,
    data: Vec<bool>,
}

impl InputHostGenerator {
    pub fn new(localhost: String, remotehost: String, total_runs: usize, ratio_local: f64) -> Self {
        let mut rng = SmallRng::seed_from_u64(42);
        let local_count = round(ratio_local * total_runs as f64) as usize;
        let remote_count = total_runs - local_count;

        let mut data = alloc::vec![true; local_count];
        data.extend(alloc::vec![false; remote_count]);
        data.shuffle(&mut rng);

        InputHostGenerator {
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
