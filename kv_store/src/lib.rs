#![no_std]
extern crate alloc;
use alloc::{
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap},
    string::String,
    vec::Vec,
};
use core::{clone::Clone, ops::Bound};
use edfsm::{Change, Fsm};

/// A query to the KV store.
///
/// Type parameter `V` is the value type of the KV store. (The key type is `Path`.)
/// The functions of type `RespondOne` and `RespondMany` are passed the query result.
///
/// Results contain borrowed values `&V` which can't be passed to channels or
/// other data structures.  The repond function may clone these to pass them on,
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
    // Can't implement a remove command because cammands can't directly alter state.
    // Remove(Path, RespondOne<V>),
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
        let s = match r.0.entry(e.key()?) {
            Entry::Vacant(entry) => entry.insert(Default::default()),
            Entry::Occupied(entry) => entry.into_mut(),
        };

        M::on_event(s, e)
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

/// The trait for events that are dispatched by key.
pub trait Keyed {
    fn key(&self) -> Option<Path>;
}

/// The key to a KV store is a pathname, `Path`, and allows heirarchical grouping of values.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct Path {
    items: Vec<PathItem>,
}

impl Path {
    /// The length of this path.
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// One element of a `Path` can be a number or a name.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum PathItem {
    Number(u64),
    Name(String),
}
