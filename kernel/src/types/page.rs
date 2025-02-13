use crate::page::{Page as AnyPage, Page1GB, Page2MB, Page4KB};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Page {
    Page4KB(AnyPage<Page4KB>),
    Page2MB(AnyPage<Page2MB>),
    Page1GB(AnyPage<Page1GB>),
}
