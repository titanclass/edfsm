#![no_std]

pub mod path;
pub use path::Path;

#[cfg(feature = "tokio")]
pub mod ask;
#[cfg(feature = "tokio")]
pub use ask::Ask;

extern crate alloc;
use alloc::{
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap},
};
use core::{clone::Clone, ops::Bound};
use edfsm::{Change, Drain, Fsm, Init, Input, Terminating};

/// The event type of an Fsm
pub type Event<M> = <M as Fsm>::E;

/// The command type of an Fsm
pub type Command<M> = <M as Fsm>::C;

/// The input type of an Fsm
pub type In<M> = Input<<M as Fsm>::C, <M as Fsm>::E>;

/// The output message type of an Fsm for the purpose of this module.
pub type Out<M> = <<M as Fsm>::SE as Drain>::Item;

/// The effector/effects type of an Fsm
pub type Effect<M> = <M as Fsm>::SE;

/// The state type of an Fsm
pub type State<M> = <M as Fsm>::S;

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
type RespondMany<V> = Box<dyn FnOnce(&dyn Iterator<Item = (&Path, &V)>) + Send>;

/// Type of a function that will respond to a single valued query response
type RespondOne<V> = Box<dyn FnOnce(Option<&V>) + Send>;

/// `KvStore<M>` represents the collection of state machines of type `M`.
///
/// `KvStore<M>` implements `Fsm` by distributing events to
/// the machines in contains by key.
/// The event type must implement trait `Keyed` which provides a key
/// for each event or type `Path`.
///
/// Commands are used to query and manager the store.  
pub struct KvStore<M>(BTreeMap<Path, State<M>>)
where
    M: Fsm;

impl<M> Fsm for KvStore<M>
where
    M: Fsm + 'static,
    State<M>: Default,
    Event<M>: Terminating,
    Effect<M>: Drain,
{
    type S = Self;
    type C = Query<State<M>>;
    type E = Keyed<Event<M>>;
    type SE = Keyed<Effect<M>>;

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
        match (r.0.entry(e.key.clone()), e.item.terminating()) {
            (Occupied(entry), false) => {
                let s = entry.into_mut();
                M::on_event(s, &e.item)
            }
            (Vacant(entry), false) => {
                let s = entry.insert(Default::default());
                M::on_event(s, &e.item)
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
            let s = r.0.get(&e.key)?;
            se.key = e.key.clone();
            M::on_change(s, &e.item, &mut se.item, change);
            Some(())
        };
        f();
    }
}

/// This type pairs a `Path` with another value.
/// This may be an event or output of a state machine
/// in the KvStore.
#[derive(Clone, Debug)]
pub struct Keyed<A> {
    pub key: Path,
    pub item: A,
}

impl<M> Default for KvStore<M>
where
    M: Fsm,
{
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<SE> Drain for Keyed<SE>
where
    SE: Drain,
{
    type Item = Keyed<SE::Item>;

    fn drain_all(&mut self) -> impl Iterator<Item = Self::Item> + Send {
        self.item.drain_all().map(|item| Keyed {
            key: self.key.clone(),
            item,
        })
    }
}

impl<S, SE> Init<S> for Keyed<SE> {
    fn init(&mut self, _state: &S) {}
}

impl<SE> Default for Keyed<SE>
where
    SE: Default,
{
    fn default() -> Self {
        Self {
            key: Default::default(),
            item: Default::default(),
        }
    }
}
