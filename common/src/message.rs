#[derive(Clone)]
pub enum OpCode {
    CreateBlob,
}

#[derive(Clone)]
pub struct Message {
    pub opcode: OpCode,
    pub ptr: usize,
    pub size: usize,
}
