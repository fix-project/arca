use crate::page::{Page1GB, Page2MB, Page4KB, SharedPage};

#[derive(Clone)]
pub enum Page {
    Page4KB(SharedPage<Page4KB>),
    Page2MB(SharedPage<Page2MB>),
    Page1GB(SharedPage<Page1GB>),
}
