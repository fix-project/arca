use crate::shell::Value;
use alloc::rc::Rc;
use fixwasm::{Error, Value as AsValue};

/// A dependency-graph node, keyed by operation and input identity.
#[derive(Clone)]
pub struct Node {
    input: Rc<Value>,
    key: (u64, [u8; 32]),
}

impl Node {
    /// Borrow the input whose identity names this graph node.
    pub fn input(&self) -> &Value {
        &self.input
    }

    fn new(code: u64, input: &Value) -> Result<Self, Error> {
        Ok(Self {
            input: Rc::new(input.clone()),
            key: (code, input.raw_handle_bytes()?),
        })
    }
    pub fn eval(input: &Value) -> Result<Self, Error> {
        Self::new(1, input)
    }
    pub fn apply(input: &Value) -> Result<Self, Error> {
        Self::new(2, input)
    }
    pub fn force(input: &Value) -> Result<Self, Error> {
        Self::new(3, input)
    }
    pub fn think(input: &Value) -> Result<Self, Error> {
        Self::new(4, input)
    }
    pub fn select(input: &Value) -> Result<Self, Error> {
        Self::new(6, input)
    }
    pub fn is_eval(&self) -> bool {
        self.key.0 == 1
    }
    pub fn is_apply(&self) -> bool {
        self.key.0 == 2
    }
    pub fn is_think(&self) -> bool {
        self.key.0 == 4
    }
    pub fn is_select(&self) -> bool {
        self.key.0 == 6
    }
    pub fn code(&self) -> u64 {
        self.key.0
    }
    pub(crate) async fn dispatch(
        &self,
        policy: &(impl super::Scheduler + ?Sized),
    ) -> Result<Value, Error> {
        let evaluation = crate::progress::evaluation(self.key.0, &self.input);
        let result = match self.key.0 {
            1 => policy.eval(&self.input).await,
            2 => policy.apply(&self.input).await,
            3 => policy.force(&self.input).await,
            4 => policy.think(&self.input).await,
            6 => policy.select(&self.input).await,
            _ => unreachable!(),
        };
        evaluation.finish(result)
    }
}
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl Eq for Node {}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Node {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}
