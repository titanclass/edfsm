use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use derive_more::derive::From;

/// The key to a KV store is a pathname, `Path`, and allows heirarchical grouping of values.
/// A path can be constructed with an expression such as:
///
///  `Path::root().append("first_level").append(42),append("third_level")`
///
/// or imperatively using `path.push(item)`.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug, Default)]
pub struct Path {
    items: Vec<PathItem>,
}

impl Path {
    /// Another name for the empty path, also the default path.
    pub fn root() -> Self {
        Self::default()
    }

    /// Append an item to the path
    pub fn append(mut self, item: impl Into<PathItem>) -> Self {
        self.push(item.into());
        self
    }

    /// Push a `PathItem` to the end of this path
    pub fn push(&mut self, item: PathItem) {
        self.items.push(item);
    }

    /// The length of this path.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// This is the empty or root path.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// One element of a `Path` can be a number or a name.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug, From)]
pub enum PathItem {
    Number(u64),
    Name(String),
}

impl From<&str> for PathItem {
    fn from(value: &str) -> Self {
        value.to_string().into()
    }
}