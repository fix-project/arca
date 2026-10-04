/**
 * This file contains Rust *pseudocode* corresponding to the Fix object hierarchy.
 * It is not expected to compile or run directly.
 */

/* Objects */

impl Handle<Object<Blob>> {
    pub fn create(bytes: &[u8]) -> Handle<Object<Blob>>;
    pub fn read(&self) -> &[u8];
}

impl Handle<Object<Tree>> {
    pub fn create(bytes: &[Handle<Any>]) -> Handle<Object<Tree>>;
    pub fn read(&self) -> &[Handle<Any>];
}

impl<T> From<Handle<Object<T>>> for Handle<Object<Any>> {}

/* Refs */

impl<T> Handle<Ref<T>>: From<Handle<Object<T>>> {
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

impl<T> From<Handle<Ref<T>>> for Handle<Ref<Any>> {}

/* Thunks */

impl Handle<Thunk> {
    pub fn application(combination: Handle<Ref<Tree>>) -> Handle<Thunk>
    pub fn identification<T>(combination: Handle<T>) -> Handle<Thunk>
    pub fn selection(combination: Handle<Ref<Tree>>) -> Handle<Thunk>
}

/* Encode */

impl Handle<Encode> {
    pub fn strict(thunk: Handle<Thunk>) -> Handle<Encode>
    pub fn shallow(thunk: Handle<Thunk>) -> Handle<Encode>
    pub fn is_strict(&self) -> bool;
    pub fn is_shallow(&self) -> bool;
}

impl From<Handle<T>> for Handle<Any> {}
