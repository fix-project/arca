// new data structure to keep track of handle as well as what to do

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operation {
    Evaluate,
    Force,
    Apply,
    Select,
}

#[derive(Clone, Copy, Debug)]
pub struct Computation<H: WorkHandle = Handle> {
    operation: Operation,
    input: H,
}