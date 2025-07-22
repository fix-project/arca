#[derive(Debug, Copy, Clone, Default)]
#[repr(C, packed)]
pub struct Header {
    pub src_cid: u64,
    pub dst_cid: u64,
    pub src_port: u32,
    pub dst_port: u32,
    pub len: u32,
    pub ptype: u16,
    pub op: u16,
    pub flags: u32,
    pub buf_alloc: u32,
    pub fwd_cnt: u32,
}

const _: () = const {
    assert!(core::mem::size_of::<Header>() == 44);
};
