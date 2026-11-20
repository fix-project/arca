use crate::{
    abi::{coordinate, effect},
    shell::{Value, capture, fields},
};
use alloc::collections::{BTreeMap, BTreeSet};
use core::cell::RefCell;
use fixwasm::Value as AsValue;

struct Labels(RefCell<Option<BTreeSet<[u8; 32]>>>);
struct Active(RefCell<BTreeMap<(u64, [u8; 32]), usize>>);
// One kernel thread polls this scheduler instance.
unsafe impl Sync for Labels {}
unsafe impl Sync for Active {}
static LABELS: Labels = Labels(RefCell::new(None));
static ACTIVE: Active = Active(RefCell::new(BTreeMap::new()));

pub struct Evaluation {
    task: Option<(Value, u64, [u8; 32])>,
    success: Option<bool>,
}

impl Evaluation {
    pub fn finish<T, E>(mut self, result: Result<T, E>) -> Result<T, E> {
        self.success = Some(result.is_ok());
        result
    }
}
impl Drop for Evaluation {
    fn drop(&mut self) {
        if let Some((input, operation, name)) = &self.task {
            let mut active = ACTIVE.0.borrow_mut();
            let count = active.get_mut(&(*operation, *name)).unwrap();
            *count -= 1;
            if *count == 0 {
                active.remove(&(*operation, *name));
                if let Ok(operation) = capture(*operation) {
                    let outcome = match self.success {
                        Some(true) => "completed",
                        Some(false) => "failed",
                        None => "cancelled",
                    };
                    if let Ok(outcome) = capture(fixwasm::Blob::create(outcome)) {
                        let _ =
                            coordinate(b"evaluation_finish", &[input.clone(), operation, outcome]);
                    }
                }
            }
        }
    }
}

pub(crate) fn named(mut name: [u8; 32]) -> Result<bool, fixwasm::Error> {
    if LABELS.0.borrow().is_none() {
        let values = effect(b"evaluation_labels", &[])?;
        let mut labels = BTreeSet::new();
        for value in fields(&values)? {
            let mut name = [0; 32];
            value.read_into(0, &mut name)?;
            name[31] &= 0xe0;
            labels.insert(name);
        }
        *LABELS.0.borrow_mut() = Some(labels);
    }
    name[31] &= 0xe0;
    Ok(LABELS.0.borrow().as_ref().unwrap().contains(&name))
}

/// Report the lifetime of an operation on a value named in the repository.
pub fn evaluation(operation: u64, input: &Value) -> Evaluation {
    let result = input.raw_handle_bytes().and_then(named);
    if result.unwrap_or(false) {
        self::operation(operation, input)
    } else {
        Evaluation {
            task: None,
            success: None,
        }
    }
}

/// Report an operation even when its input has no repository label.
pub(crate) fn operation(operation: u64, input: &Value) -> Evaluation {
    if operation == 1 && input.is_value().unwrap_or(false) {
        return Evaluation {
            task: None,
            success: None,
        };
    }
    let started = (|| -> Result<[u8; 32], fixwasm::Error> {
        let name = input.raw_handle_bytes()?;
        let mut active = ACTIVE.0.borrow_mut();
        let count = active.entry((operation, name)).or_default();
        if *count == 0 {
            coordinate(b"evaluation_start", &[input.clone(), capture(operation)?])?;
        }
        *count += 1;
        Ok(name)
    })();
    Evaluation {
        task: started.ok().map(|name| (input.clone(), operation, name)),
        success: None,
    }
}

pub(crate) struct Load(Option<Value>);

impl Load {
    pub(crate) fn finish(mut self, success: bool, bytes: Option<u64>) {
        self.end(if success { "completed" } else { "failed" }, bytes);
    }

    fn end(&mut self, outcome: &str, bytes: Option<u64>) {
        if let Some(id) = self.0.take()
            && let (Ok(outcome), Ok(bytes)) = (
                capture(fixwasm::Blob::create(outcome)),
                capture(bytes.unwrap_or(u64::MAX)),
            )
        {
            let _ = coordinate(b"load_finish", &[id, outcome, bytes]);
        }
    }
}

impl Drop for Load {
    fn drop(&mut self) {
        self.end("cancelled", None);
    }
}

/// Report a transfer's value and endpoints without choosing a display format.
pub(crate) fn load(input: &Value, source: &str, destination: &str, peer: Option<[u8; 16]>) -> Load {
    let started = (|| {
        effect(
            b"load_start",
            &[
                input.clone(),
                capture(fixwasm::Blob::create(source))?,
                capture(fixwasm::Blob::create(destination))?,
                capture(fixwasm::Blob::create(
                    peer.as_ref().map_or(&[][..], |id| &id[..]),
                ))?,
            ],
        )
    })();
    Load(started.ok())
}

/// End a reported operation when its result was reused.
pub fn cached(operation: u64, input: &Value) -> Result<(), fixwasm::Error> {
    coordinate(
        b"evaluation_finish",
        &[
            input.clone(),
            capture(operation)?,
            capture(fixwasm::Blob::create(b"cached"))?,
        ],
    )?;
    Ok(())
}
