use alloc::{string::String, vec::Vec};
use core::{
    fmt::{Display, Write as _},
    ops::Div,
    slice::Iter,
    str::FromStr,
};
use derive_more::{
    derive::{Deref, IntoIterator},
    From, TryInto,
};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

const URL_COMPONENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'|');

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

impl Display for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut buffer = String::new();
        for item in self.iter() {
            buffer.push('/');
            match item {
                PathItem::Number(n) => {
                    write!(&mut buffer, "{n}")?;
                }
                PathItem::Name(c) => {
                    if let Some(x) = c.chars().next() {
                        if x.is_ascii_digit() || x == '\'' {
                            buffer.push('\'');
                        }
                    }
                    for segment in utf8_percent_encode(c, URL_COMPONENT) {
                        buffer.push_str(segment);
                    }
                }
            }
        }
        f.write_str(&buffer)
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    NoRoot,
    BadInt(core::num::ParseIntError),
}

impl FromStr for Path {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('/') {
            return Err(ParseError::NoRoot);
        }
        let mut path = Self::root();
        let mut raw_path_items = s.split('/');
        let _ = raw_path_items.next();
        let mut decode_buffer = String::with_capacity(s.len());
        for raw_path_item in raw_path_items {
            let mut raw_path_item_iter = raw_path_item.chars();
            let path_item = match raw_path_item_iter.next() {
                Some(c) if c.is_ascii_digit() => {
                    PathItem::Number(raw_path_item.parse().map_err(ParseError::BadInt)?)
                }
                Some('\'') => {
                    decode_buffer.push_str(
                        percent_decode_str(raw_path_item_iter.as_str())
                            .decode_utf8_lossy()
                            .as_ref(),
                    );
                    PathItem::Name(SmolStr::from(&decode_buffer))
                }
                _ => {
                    decode_buffer.push_str(
                        percent_decode_str(raw_path_item)
                            .decode_utf8_lossy()
                            .as_ref(),
                    );
                    PathItem::Name(SmolStr::from(&decode_buffer))
                }
            };
            path.push(path_item);
            decode_buffer.clear();
        }
        Ok(path)
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
    use crate::path::ParseError;

    use super::{root, Path, PathItem};
    use alloc::{format, string::ToString};
    use smol_str::SmolStr;

    #[test]
    fn path_test() {
        // bulding from various types (note: special smol_str support for &'static str engaged)
        let version = "V1.6";
        let path = root() / "CSMS" / 65 / format!("EVSE-{version}") / 2;

        // indexing
        let evse: u64 = path[3].clone().try_into().unwrap();
        assert_eq!(evse, 2);

        // indexing again
        let x: Option<SmolStr> = path[2].clone().try_into().ok();
        assert_eq!(x.unwrap(), "EVSE-V1.6");

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
        let csms: u64 = path.iter().nth(1).unwrap().clone().try_into().unwrap();
        assert_eq!(csms, 65);

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

    #[test]
    fn to_string_1() {
        let p = root() / "CS" / 1;
        assert_eq!(p.to_string(), "/CS/1");
    }

    #[test]
    fn to_string_2() {
        let p = root() / "CS/MS" / 65 / "EV?S&E" / 2;
        assert_eq!(p.to_string(), "/CS%2FMS/65/EV%3FS%26E/2");
    }

    #[test]
    fn to_string_3() {
        let p = root() / "CS" / "2";
        assert_eq!(p.to_string(), "/CS/'2");
    }

    #[test]
    fn to_string_4() {
        let p = root() / "'CS" / 2;
        assert_eq!(p.to_string(), "/''CS/2");
    }

    #[test]
    fn from_string_1() {
        let p = root() / "CS" / 1;
        assert_eq!("/CS/1".parse(), Ok(p));
    }

    #[test]
    fn from_string_2() {
        let p = root() / "CS/MS" / 65 / "EV?S&E" / 2;
        assert_eq!("/CS%2FMS/65/EV%3FS%26E/2".parse(), Ok(p));
    }

    #[test]
    fn from_string_3() {
        let p = root() / "CS" / "2";
        assert_eq!("/CS/'2".parse(), Ok(p));
    }

    #[test]
    fn from_string_4() {
        let p = root() / "'CS" / 2;
        assert_eq!("/''CS/2".parse(), Ok(p));
    }

    #[test]
    fn unicode_round_trip() {
        let p = root() / "中文/🚀" / 7;
        let encoded = p.to_string();
        assert_eq!(encoded, "/%E4%B8%AD%E6%96%87%2F%F0%9F%9A%80/7");
        assert_eq!(encoded.parse::<Path>(), Ok(p));
    }

    #[test]
    fn from_string_no_root_err() {
        assert_eq!("CS".parse::<Path>(), Err(ParseError::NoRoot));
    }

    #[test]
    fn from_string_bad_int_err() {
        assert!(matches!(
            "/1n".parse::<Path>(),
            Err(ParseError::BadInt(core::num::ParseIntError { .. }))
        ));
    }
}
