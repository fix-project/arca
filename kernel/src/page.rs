use crate::prelude::*;
use common::BuddyAllocator;

pub trait HardwarePage: Sized + Clone {}
impl HardwarePage for Page4KB {}
impl HardwarePage for Page2MB {}
impl HardwarePage for Page1GB {}
impl HardwarePage for ! {}

pub type Page4KB = [u8; 1 << 12];
pub type Page2MB = [u8; 1 << 21];
pub type Page1GB = [u8; 1 << 30];

pub type UniquePage<T> = Box<T, &'static BuddyAllocator<'static>>;
pub type SharedPage<T> = RefCnt<'static, T>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Page<T> {
    Unique(UniquePage<T>),
    Shared(SharedPage<T>),
}
