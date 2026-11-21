use core::fmt;

pub enum FailureKind {
    Error,
    Panic,
    Trap,
    Memory,
}

impl FailureKind {
    pub fn from_marker(marker: &[u8]) -> Option<Self> {
        Some(match marker {
            b"Fix.error" => Self::Error,
            b"Rust.panic" => Self::Panic,
            b"Wasm.trap" => Self::Trap,
            b"Fix.memory" => Self::Memory,
            _ => return None,
        })
    }

    pub fn decode(self, detail: &[u8]) -> Option<ExecutionFailure<'_>> {
        Some(match self {
            Self::Error => ExecutionFailure::Error(detail),
            Self::Panic => ExecutionFailure::Panic(detail),
            Self::Trap => ExecutionFailure::Trap(u32::from_le_bytes(detail.try_into().ok()?)),
            Self::Memory => ExecutionFailure::Memory(u64::from_le_bytes(detail.try_into().ok()?)),
        })
    }
}

/// A conventional Fix execution error decoded from the two Blobs of a result Tree.
pub enum ExecutionFailure<'a> {
    Error(&'a [u8]),
    Panic(&'a [u8]),
    Trap(u32),
    Memory(u64),
}

impl fmt::Display for ExecutionFailure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let detail = match self {
            Self::Error(detail) => {
                f.write_str("Fix error: ")?;
                detail
            }
            Self::Panic(detail) => {
                f.write_str("Rust panic: ")?;
                detail
            }
            Self::Trap(code) => return write!(f, "Wasm trap code {code}"),
            Self::Memory(limit) => return write!(f, "memory limit exceeded ({limit} bytes)"),
        };
        for chunk in detail.utf8_chunks() {
            f.write_str(chunk.valid())?;
            if !chunk.invalid().is_empty() {
                f.write_str("�")?;
            }
        }
        Ok(())
    }
}
