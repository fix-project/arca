pub const PAGE: usize = 4096;
pub const RECORD: usize = 16384;
pub const DESCRIPTOR: usize = 128;
pub const WORKING: usize = 8192;
pub const INVOCATION: usize = (2 << 20) + 16384;
pub const BUDGET: usize = 65536;

pub fn mapping(bytes: usize) -> usize {
    bytes
        .div_ceil(PAGE)
        .saturating_mul(PAGE)
        .saturating_add(mapping_bookkeeping(0, bytes))
}

pub fn mapping_bookkeeping(address: usize, bytes: usize) -> usize {
    let pages = bytes.div_ceil(PAGE);
    const TABLE: usize = 16 << 10;
    pages
        .saturating_mul(128)
        .saturating_add(table_count(address, bytes, 2 << 20).saturating_mul(TABLE))
        .saturating_add(table_count(address, bytes, 1 << 30).saturating_mul(TABLE))
        .saturating_add(table_count(address, bytes, 1 << 39).saturating_mul(TABLE))
        .min(pages.saturating_mul(4 * PAGE))
}

pub fn allocation(bytes: usize) -> usize {
    bytes
        .max(PAGE)
        .checked_next_power_of_two()
        .unwrap_or(usize::MAX)
}

pub fn object(bytes: usize) -> usize {
    mapping(bytes).saturating_add(RECORD)
}

pub fn tree_encoding(length: usize) -> usize {
    if length == 0 {
        0
    } else {
        allocation(length.saturating_mul(32)).saturating_add(RECORD)
    }
}

pub fn blob_encoding(bytes: usize) -> usize {
    tree_encoding(2).saturating_add(if bytes <= 30 {
        0
    } else {
        allocation(bytes).saturating_add(RECORD)
    })
}

pub fn table_encoding() -> usize {
    tree_encoding(2)
        .saturating_mul(2)
        .saturating_add(tree_encoding(512))
}

pub fn encoded_leaves(bytes: usize) -> usize {
    bytes
        .div_ceil(PAGE)
        .saturating_mul(blob_encoding(PAGE).saturating_add(tree_encoding(3)))
}

fn table_count(address: usize, bytes: usize, span: u64) -> usize {
    if bytes == 0 {
        return 0;
    }
    let last = address.saturating_add(bytes - 1);
    (last as u64 / span - address as u64 / span) as usize + 1
}

pub fn encoded_nodes(address: usize, bytes: usize) -> usize {
    let nodes =
        table_count(address, bytes, 2 << 20).saturating_add(table_count(address, bytes, 1 << 30));
    nodes.saturating_mul(table_encoding().saturating_add(tree_encoding(3)))
}

pub fn encoded_mapping(address: usize, bytes: usize) -> usize {
    encoded_leaves(bytes).saturating_add(encoded_nodes(address, bytes))
}

pub fn encoded_metadata(bytes: usize) -> usize {
    let pages = bytes.div_ceil(PAGE);
    pages.saturating_mul(
        blob_encoding(PAGE)
            .saturating_add(tree_encoding(3))
            .saturating_add(2 * table_encoding().saturating_add(tree_encoding(3))),
    )
}
