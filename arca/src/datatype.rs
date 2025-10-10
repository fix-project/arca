#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DataType {
    Null,
    Word,
    Blob,
    Tuple,
    Page,
    Table,
    Function,
}
