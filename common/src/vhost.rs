#[repr(C)]
#[derive(Copy, Clone)]
pub struct VSockMetadata {
    pub rx: VirtQueueMetadata,
    pub tx: VirtQueueMetadata,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VirtQueueMetadata {
    pub descriptors: usize,
    pub desc: usize,
    pub used: usize,
    pub avail: usize,
}
