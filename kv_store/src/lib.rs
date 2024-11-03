#![no_std]
extern crate alloc;
use alloc::{
    collections::{btree_map::Entry, BTreeMap},
    string::String,
    vec::Vec,
};
use edfsm::{Change, Fsm};

/// A command to query or manage the KV store.
#[derive(Debug, Clone)]
pub enum KvOperation {}

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
    type C = KvOperation;
    type E = M::E;
    type SE = M::SE;

    fn for_command(_r: &Self::S, _c: Self::C, _se: &mut Self::SE) -> Option<Self::E> {
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
        let f = || {
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

/// One element of a `Path` can be a number or a name.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum PathItem {
    Number(u64),
    Name(String),
}
