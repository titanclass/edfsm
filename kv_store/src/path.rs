use alloc::{string::String, vec::Vec};
use core::{ops::Div, slice::Iter};
use derive_more::{
    derive::{Deref, IntoIterator},
    From, TryInto,
};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// The key to a KV store is a pathname, `Path`, and allows heirarchical grouping of values.
/// A path can be constructed with an expression such as:
///
///  `Path::root().append("first_level").append(42),append("third_level")`
///
/// or imperatively using `path.push(item)`.
#[derive(
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    Hash,
    IntoIterator,
    Deref,
)]
#[deref(forward)]
pub struct Path(Vec<PathItem>);

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
        self.0.push(item);
    }

    /// The length of this path.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// This is the empty or root path.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> Iter<'_, PathItem> {
        self.0.iter()
    }
}

/// Another name for the empty path, also the default path.
pub fn root() -> Path {
    Path::default()
}

impl<T> Div<T> for Path
where
    T: Into<PathItem>,
{
    type Output = Path;

    fn div(self, item: T) -> Self::Output {
        self.append(item)
    }
}

/// One element of a `Path` can be a number or a name.
#[derive(
    PartialEq, Eq, PartialOrd, Ord, Clone, Debug, From, Serialize, Deserialize, Hash, TryInto,
)]
#[serde(untagged)]
pub enum PathItem {
    Number(u64),
    Name(SmolStr),
}

impl From<&'static str> for PathItem {
    fn from(value: &'static str) -> Self {
        SmolStr::new_static(value).into()
    }
}

impl From<String> for PathItem {
    fn from(value: String) -> Self {
        SmolStr::new(value).into()
    }
}

#[cfg(test)]
mod test {
    use super::{root, Path, PathItem};
    use alloc::format;

    #[test]
    fn path_test() {
        // bulding from various types (note: special smol_str support for &'static str engaged)
        let version = "V1.6";
        let path = root() / "CSMS" / 65 / format!("EVSE-{version}") / 2;

        // indexing
        let evse: u64 = path[3].clone().try_into().unwrap();
        assert_eq!(evse, 2);

        // other slice operations
        if let Some(&PathItem::Number(evse)) = path.last() {
            assert_eq!(evse, 2);
        } else {
            panic!("test failed")
        }

        // pattern matching
        match &*path {
            [PathItem::Name(x), PathItem::Number(csms), ..] if x == "CSMS" => {
                assert_eq!(*csms, 65)
            }
            _ => panic!("test failed"),
        }

        // iterating
        let cmsms: u64 = path
            .iter()
            .skip(1)
            .next()
            .unwrap()
            .clone()
            .try_into()
            .unwrap();
        assert_eq!(cmsms, 65);

        // iterating
        for item in path {
            if let PathItem::Number(csms) = item {
                assert_eq!(csms, 65);
                break;
            }
        }
    }

    #[test]
    fn path_serialisation_json() {
        let p = root() / "CSMS" / 65 / "EVSE" / 2;
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(s, r#"["CSMS",65,"EVSE",2]"#);
    }

    #[test]
    fn path_deserialisation_json() {
        let s = r#"["CSMS",65,"EVSE",2]"#;
        let p: Path = serde_json::from_str(s).unwrap();
        assert_eq!(p, root() / "CSMS" / 65 / "EVSE" / 2);
    }

    #[test]
    fn path_serialisation_qs() {
        let p = root() / "CSMS" / 65 / "EVSE" / 2;
        let s = serde_qs::to_string(&p).unwrap();
        assert_eq!(s, "0=CSMS&1=65&2=EVSE&3=2");
    }
}
