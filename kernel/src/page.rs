use common::{refcnt::RefCnt, BuddyAllocator};
extern crate alloc;
use alloc::boxed::Box;

pub trait HardwarePage: Sized + Clone {}
impl HardwarePage for Page4KB {}
impl HardwarePage for Page2MB {}
impl HardwarePage for Page1GB {}
impl HardwarePage for ! {}

pub type Page4KB = [u8; 1 << 12];
pub type Page2MB = [u8; 1 << 21];
pub type Page1GB = [u8; 1 << 30];

#[allow(type_alias_bounds)]
pub type UniquePage<T: HardwarePage> = Box<T, &'static BuddyAllocator<'static>>;

#[allow(type_alias_bounds)]
pub type SharedPage<T: HardwarePage> = RefCnt<'static, T>;
