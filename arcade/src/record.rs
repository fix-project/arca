use core::{fmt::Display, time::Duration};
use derive_more::{Add, AddAssign, From};

fn average(d: &Duration, count: usize) -> Duration {
    Duration::from_micros(d.as_micros() as u64 / count as u64)
}

#[derive(Debug, Copy, Clone, Default)]
pub struct LocalRecord;

#[derive(Debug, Copy, Clone, Default, Add, AddAssign)]
pub struct RemoteDataRecord {
    pub remote_data_read: Duration,
}

impl Display for RemoteDataRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "remote_data_read: {} us",
            self.remote_data_read.as_micros()
        )
    }
}

impl RemoteDataRecord {
    fn average(&self, count: usize) -> Self {
        Self {
            remote_data_read: average(&self.remote_data_read, count),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Add, AddAssign)]
pub struct MigratedRecord {
    pub creation: Duration,
    pub serialization: Duration,
    pub compression: Duration,
    pub sending: Duration,
}

impl Display for MigratedRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "creation: {} us", self.creation.as_micros())?;
        writeln!(f, "serialization: {} us", self.serialization.as_micros())?;
        writeln!(f, "compression: {} us", self.compression.as_micros())?;
        writeln!(f, "sending: {} us", self.sending.as_micros())
    }
}

impl MigratedRecord {
    fn average(&self, count: usize) -> Self {
        Self {
            creation: average(&self.creation, count),
            serialization: average(&self.serialization, count),
            compression: average(&self.compression, count),
            sending: average(&self.compression, count),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Add, AddAssign)]
pub struct RemoteInvocationRecord {
    pub decompression: Duration,
    pub deserialization: Duration,
    pub execution: Duration,
}

impl Display for RemoteInvocationRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "decompression: {} us", self.decompression.as_micros())?;
        writeln!(
            f,
            "deserialization: {} us",
            self.deserialization.as_micros()
        )?;
        writeln!(f, "execution: {} us", self.execution.as_micros())
    }
}

impl RemoteInvocationRecord {
    fn average(&self, count: usize) -> Self {
        Self {
            decompression: average(&self.decompression, count),
            deserialization: average(&self.deserialization, count),
            execution: average(&self.execution, count),
        }
    }
}

#[derive(Debug, Copy, Clone, From)]
pub enum Record {
    LocalRecord(LocalRecord),
    RemoteDataRecord(RemoteDataRecord),
    MigratedRecord(MigratedRecord),
    RemoteInvocationRecord(RemoteInvocationRecord),
}

#[derive(Debug, Copy, Clone, Default, Add, AddAssign)]
pub struct Accumulator {
    local_record: Duration,
    remote_data_record: RemoteDataRecord,
    migrated_record: MigratedRecord,
    remote_invocation_record: RemoteInvocationRecord,
    local_count: usize,
    remote_data_count: usize,
    pub migrated_count: usize,
    remote_invocation_count: usize,
}

impl Accumulator {
    pub fn accumulate(&mut self, r: Record, total: Duration) {
        match r {
            Record::LocalRecord(_) => {
                self.local_record += total;
                self.local_count += 1;
            }
            Record::RemoteDataRecord(remote_data_record) => {
                self.remote_data_record += remote_data_record;
                self.remote_data_count += 1;
            }
            Record::MigratedRecord(migrated_record) => {
                self.migrated_record += migrated_record;
                self.migrated_count += 1;
            }
            Record::RemoteInvocationRecord(remote_invocation_record) => {
                self.remote_invocation_record += remote_invocation_record;
                self.remote_invocation_count += 1;
            }
        }
    }
}

impl Display for Accumulator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.local_count > 0 {
            write!(
                f,
                "Local execution (total {}):\n{} us",
                self.local_count,
                self.local_record.as_micros() as usize / self.local_count
            )?;
        }
        if self.remote_data_count > 0 {
            write!(
                f,
                "Local execution with remote data (total {}):\n{}",
                self.remote_data_count,
                self.remote_data_record.average(self.remote_data_count),
            )?;
        }
        if self.migrated_count > 0 {
            write!(
                f,
                "Execution migrated to another machine (total {}):\n{}",
                self.migrated_count,
                self.migrated_record.average(self.migrated_count)
            )?;
        }
        if self.remote_invocation_count > 0 {
            write!(
                f,
                "Execution from received continuation (total {}):\n{}",
                self.remote_invocation_count,
                self.remote_invocation_record
                    .average(self.remote_invocation_count),
            )?;
        }
        Ok(())
    }
}
