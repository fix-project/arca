use core::{
    borrow::Borrow,
    ops::{Add, Deref},
};

use alloc::{borrow::ToOwned, boxed::Box, string::String};

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PathBuf {
    path: String,
}

#[allow(unused)]
impl Default for PathBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl PathBuf {
    pub fn as_path(&self) -> &Path {
        self
    }

    pub fn capacity(&self) -> usize {
        self.path.capacity()
    }

    pub fn clear(&mut self) {
        self.path.clear()
    }

    pub fn into_boxed_path(self) -> Box<Path> {
        unsafe { Box::from_raw(Box::into_raw(self.path.into_boxed_str()) as *mut Path) }
    }

    pub fn into_string(self) -> String {
        self.path
    }

    pub fn new() -> Self {
        Self {
            path: "".to_owned(),
        }
    }

    pub fn pop(&mut self) -> bool {
        if self.path.is_empty() {
            false
        } else if let Some(last_sep) = self.path.rfind("/") {
            if last_sep == 0 {
                false
            } else {
                self.path.truncate(last_sep);
                true
            }
        } else {
            self.path.clear();
            true
        }
    }

    pub fn push<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if path.is_absolute() {
            *self = path.to_owned();
        } else {
            self.path.push_str(path.as_ref());
        }
    }
}

impl From<String> for PathBuf {
    fn from(value: String) -> Self {
        PathBuf { path: value }
    }
}

impl AsRef<String> for PathBuf {
    fn as_ref(&self) -> &String {
        &self.path
    }
}

impl AsRef<Path> for PathBuf {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl Deref for PathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        Path::new(&self.path)
    }
}

impl Borrow<Path> for PathBuf {
    fn borrow(&self) -> &Path {
        Path::new(&self.path)
    }
}

impl Borrow<str> for PathBuf {
    fn borrow(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Path {
    path: str,
}

impl Path {
    pub fn new<S: AsRef<str> + ?Sized>(s: &S) -> &Path {
        unsafe { &*(s.as_ref() as *const str as *const Path) }
    }

    pub fn is_absolute(&self) -> bool {
        self.path.starts_with("/")
    }

    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    pub fn components(&'_ self) -> Components<'_> {
        Components {
            root: self.is_absolute(),
            path: &self.path,
        }
    }

    pub fn starts_with(&self, base: impl AsRef<Path>) -> bool {
        let base = base.as_ref();
        let mut a = self.components();
        let mut b = base.components();
        loop {
            let x = a.next();
            let y = b.next();
            match (x, y) {
                (Some(x), Some(y)) if x == y => continue,
                (Some(_), Some(_)) => return false,
                (None, Some(_)) => return false,
                (Some(_), None) => return true,
                (None, None) => return true,
            }
        }
    }

    pub fn parent(&self) -> Option<&Path> {
        let Some((parent, _)) = self.path.rsplit_once("/") else {
            if self.path.is_empty() {
                return None;
            } else {
                return Some("".as_ref());
            }
        };
        if parent.is_empty() {
            None
        } else {
            Some(parent.as_ref())
        }
    }

    pub fn file_name(&self) -> Option<&str> {
        let Some((_, child)) = self.path.rsplit_once("/") else {
            return Some(self.as_ref());
        };
        Some(child)
    }

    pub fn strip_prefix(&self, base: impl AsRef<Path>) -> Option<&Path> {
        let base = base.as_ref();
        let mut a = self.components();
        let mut b = base.components();
        loop {
            if let Some(y) = b.next() {
                if let Some(x) = a.next()
                    && x == y
                {
                    continue;
                }
                return None;
            } else {
                return Some(a.as_path());
            }
        }
    }

    pub fn relative(&self) -> &Path {
        if self.is_absolute() {
            self.strip_prefix("/").unwrap()
        } else {
            self
        }
    }

    pub fn to_str(&self) -> &str {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.path.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn split(&self) -> (&str, &Path) {
        let mut c = self.components();
        let head = c.next();
        match head {
            Some(Component::CurDir) => (".", c.as_path()),
            Some(Component::ParentDir) => ("..", c.as_path()),
            Some(Component::RootDir) => ("/", c.as_path()),
            Some(Component::Normal(x)) => (x, c.as_path()),
            None => ("", c.as_path()),
        }
    }
}

impl AsRef<str> for Path {
    fn as_ref(&self) -> &str {
        &self.path
    }
}

impl AsRef<Path> for str {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for String {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for Path {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl ToOwned for Path {
    type Owned = PathBuf;

    fn to_owned(&self) -> Self::Owned {
        PathBuf {
            path: self.path.to_owned(),
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Component<'a> {
    RootDir,
    CurDir,
    ParentDir,
    Normal(&'a str),
}

impl AsRef<str> for Component<'_> {
    fn as_ref(&self) -> &str {
        match self {
            Component::RootDir => "/",
            Component::CurDir => ".",
            Component::ParentDir => "..",
            Component::Normal(x) => x,
        }
    }
}

impl AsRef<Path> for Component<'_> {
    fn as_ref(&self) -> &Path {
        Path::new::<str>(self.as_ref())
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Components<'a> {
    root: bool,
    path: &'a str,
}

impl<'a> Components<'a> {
    pub fn as_path(&self) -> &'a Path {
        if self.root && self.path.is_empty() {
            Path::new("/")
        } else {
            Path::new(self.path)
        }
    }
}

impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.path.is_empty() {
            return None;
        };
        let root = self.root;
        self.root = false;
        let Some((current, next)) = self.path.split_once("/") else {
            let path = self.path;
            self.path = "";
            return Some(Component::Normal(path));
        };
        self.path = next;
        Some(match current {
            "" => {
                if root {
                    Component::RootDir
                } else {
                    return self.next();
                }
            }
            "." => Component::CurDir,
            ".." => Component::ParentDir,
            x => Component::Normal(x),
        })
    }
}

impl<'a> DoubleEndedIterator for Components<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.path.is_empty() {
            return None;
        };
        let Some((current, next)) = self.path.rsplit_once("/") else {
            let path = self.path;
            let root = self.root;
            self.root = false;
            if root {
                return None;
            }
            self.path = "";
            return Some(Component::Normal(path));
        };
        self.path = current;
        Some(match next {
            "" => {
                return self.next_back();
            }
            "." => Component::CurDir,
            ".." => Component::ParentDir,
            x => Component::Normal(x),
        })
    }
}

impl Add<&str> for PathBuf {
    type Output = PathBuf;

    fn add(mut self, rhs: &str) -> Self::Output {
        self.path += rhs;
        self
    }
}

impl Add<&Path> for PathBuf {
    type Output = PathBuf;

    fn add(mut self, rhs: &Path) -> Self::Output {
        self.path += &rhs.path;
        self
    }
}

impl core::fmt::Display for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", &self.path)
    }
}

impl core::fmt::Display for PathBuf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.path)
    }
}
