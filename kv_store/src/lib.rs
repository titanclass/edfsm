#![no_std]
extern crate alloc;
use alloc::{
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap},
    string::{String, ToString},
    vec::Vec,
};
use core::{clone::Clone, ops::Bound};
use derive_more::From;
use edfsm::{Change, Fsm};

/// A query to the KV store.
///
/// Type parameter `V` is the value type of the KV store. (The key type is `Path`.)
/// The functions of type `RespondOne` and `RespondMany` are passed the query result.
///
/// Results contain borrowed values `&V` which can't be passed to channels or
/// other data structures.  The respond function may clone these to pass them on,
/// or the function may interpret or aggregate borrowed values in place.
///
pub enum Query<V> {
    /// Get the value at the given path
    Get(Path, RespondOne<V>),

    /// Get the entries whose path starts with the given path,
    /// including the entry for the path itself.
    GetTree(Path, RespondMany<V>),

    /// Get the entries in the given range
    GetRange((Bound<Path>, Bound<Path>), RespondMany<V>),

    /// Get all the entries
    GetAll(RespondMany<V>),
}

/// Type of a function that will respond to an iterator over query results.
type RespondMany<V> = Box<dyn FnOnce(&dyn Iterator<Item = (&Path, &V)>)>;

/// Type of a function that will respond to a single valued query response
type RespondOne<V> = Box<dyn FnOnce(Option<&V>)>;

/// `KvStore<M>` represents the collection of state machines of type `M`.
///
/// `KvStore<M>` implements `Fsm` by distributing events to
/// the machines in contains by key.
/// The event type must implement trait `Keyed` which provides a key
/// for each event or type `Path`.
///
/// Commands are used to query and manager the store.  
#[derive(Debug, Default)]
pub struct KvStore<M>(BTreeMap<Path, M::S>)
where
    M: Fsm;

impl<M> Fsm for KvStore<M>
where
    M: Fsm + 'static,
    M::S: Default,
    M::E: Keyed,
{
    type S = Self;
    type C = Query<M::S>;
    type E = M::E;
    type SE = M::SE;

    fn for_command(store: &Self::S, command: Self::C, _se: &mut Self::SE) -> Option<Self::E> {
        use Bound::*;
        use Query::*;
        match command {
            Get(path, respond) => respond(store.0.get(&path)),
            GetTree(path, respond) => respond(
                &(store
                    .0
                    .range((Included(&path), Unbounded))
                    .take_while(|(p, _)| p.len() > path.len() || *p == &path)),
            ),
            GetRange(bounds, respond) => respond(&store.0.range(bounds)),
            GetAll(respond) => respond(&store.0.iter()),
        }
        None
    }

    fn on_event(r: &mut Self::S, e: &Self::E) -> Option<Change> {
        use Entry::*;
        match (r.0.entry(e.key()?), e.terminating()) {
            (Occupied(entry), false) => {
                let s = entry.into_mut();
                M::on_event(s, e)
            }
            (Vacant(entry), false) => {
                let s = entry.insert(Default::default());
                M::on_event(s, e)
            }
            (Occupied(entry), true) => {
                entry.remove();
                Some(Change::Transitioned)
            }
            (Vacant(_), true) => None,
        }
    }

    fn on_change(r: &Self::S, e: &Self::E, se: &mut Self::SE, change: Change) {
        let mut f = || {
            let s = r.0.get(&e.key()?)?;
            M::on_change(s, e, se, change);
            Some(())
        };
        f();
    }
}

/// A trait for events that are dispatched by key.
pub trait Keyed {
    /// This event applies to state with at the given path.
    /// If `None` the event is ignored.
    fn key(&self) -> Option<Path>;

    /// This event is the final event for the path,
    /// and the state at the path will be removed.
    fn terminating(&self) -> bool;
}

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
