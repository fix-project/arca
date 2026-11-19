pub const PAGE: usize = 4096;
pub const RECORD: usize = 16384;
pub const DESCRIPTOR: usize = 128;
pub const WORKING: usize = 8192;
pub const INVOCATION: usize = (2 << 20) + 16384;
pub const BUDGET: usize = 65536;

pub fn mapping(bytes: usize) -> usize {
    bytes.div_ceil(PAGE).saturating_mul(5 * PAGE)
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

pub fn encoded_nodes(address: usize, bytes: usize) -> usize {
    if bytes == 0 {
        return 0;
    }
    let first = address / PAGE;
    let pages = (address % PAGE).saturating_add(bytes).div_ceil(PAGE);
    let last = first.saturating_add(pages).saturating_sub(1);
    let nodes = (last / 512)
        .saturating_sub(first / 512)
        .saturating_add(1)
        .saturating_add(
            (last / (512 * 512))
                .saturating_sub(first / (512 * 512))
                .saturating_add(1),
        );
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
