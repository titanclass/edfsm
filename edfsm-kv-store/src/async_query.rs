use crate::{Keyed, Path, Query, RespondMany, RespondOne};
use alloc::boxed::Box;
use core::ops::Bound;
use edfsm::Input;
use edfsm_machine::{adapter::Adapter, error::Result};
use tokio::sync::oneshot;

/// Create a handle for async queries on the given channel or adapter
pub fn requester<T>(sender: T) -> Requester<T> {
    Requester(sender)
}

/// A handle for async queries to a `kv_store`
#[derive(Debug)]
pub struct Requester<T>(T);

impl<T, V, E> Requester<T>
where
    T: Adapter<Item = Input<Query<V, E>, Keyed<E>>>,
    V: 'static,
    E: 'static,
{
    /// Get the value at the given path.
    /// Apply `func` to this and return the result.
    pub async fn get<F, R>(&mut self, path: Path, func: F) -> Result<R>
    where
        F: FnOnce(Option<&V>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (remote, receiver) = respond_one(|v| (func(v), ()));
        self.dispatch(Query::Get(path, remote), receiver).await
    }

    /// Get the entries whose path starts with the given path,
    /// including the entry for the path itself.
    /// Apply `func` to these and return the result.
    pub async fn get_tree<F, R>(&mut self, path: Path, func: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (remote, receiver) = respond_many(|vs| (func(vs), ()));
        self.dispatch(Query::GetTree(path, remote), receiver).await
    }

    /// Get the entries in the given range
    /// Apply `func` to these and return the result.
    pub async fn get_range<F, R>(&mut self, range: (Bound<Path>, Bound<Path>), func: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (remote, receiver) = respond_many(|vs| (func(vs), ()));
        self.dispatch(Query::GetRange(range, remote), receiver)
            .await
    }

    /// Get all the entries
    /// Apply `func` to these and return the result.
    pub async fn get_all<F, R>(&mut self, func: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (remote, receiver) = respond_many(|vs| (func(vs), ()));
        self.dispatch(Query::GetAll(remote), receiver).await
    }

    /// Get the value at the given path, or none, and apply a function that produces an event.
    ///
    /// The event will be applied to the extant value or a new value at the path.
    /// The result indicates whether an extant value was found.
    pub async fn upsert<F>(&mut self, path: Path, func: F) -> Result<Extant>
    where
        F: FnOnce(Option<&V>) -> E + Send + 'static,
    {
        let (remote, receiver) = respond_one(|v| {
            let x = v.is_some().into();
            (x, func(v))
        });
        self.dispatch(Query::Upsert(path, remote), receiver).await
    }

    /// Get all the entries and apply a function that produces an event.
    ///
    /// The event is keyed for a particular path. Usually this would be a new path
    /// not found among the extant entries and new value will be created.
    /// The event will applied to the new or extant value. The path is returned.
    pub async fn insert<F>(&mut self, func: F) -> Result<Path>
    where
        F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> Keyed<E> + Send + 'static,
    {
        let (remote, receiver) = respond_many(|vs| {
            let e = func(vs);
            (e.key.clone(), e)
        });
        self.dispatch(Query::Insert(remote), receiver).await
    }

    async fn dispatch<R>(&mut self, query: Query<V, E>, rx: oneshot::Receiver<R>) -> Result<R> {
        self.0.notify(Input::Command(query)).await;
        Ok(rx.await?)
    }
}

/// Indicates whether an extant (ie existing) value is found in a `KvStore`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extant {
    Found,
    NotFound,
}

impl From<bool> for Extant {
    fn from(value: bool) -> Self {
        if value {
            Extant::Found
        } else {
            Extant::NotFound
        }
    }
}

fn respond_one<F, V, R, E>(func: F) -> (RespondOne<V, E>, oneshot::Receiver<R>)
where
    F: FnOnce(Option<&V>) -> (R, E) + Send + 'static,
    R: Send + 'static,
{
    let (sender, receiver) = oneshot::channel();
    let remote = Box::new(|v: Option<&V>| {
        let (r, e) = func(v);
        let _ = sender.send(r);
        e
    });
    (remote, receiver)
}

fn respond_many<F, V, R, E>(func: F) -> (RespondMany<V, E>, oneshot::Receiver<R>)
where
    F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> (R, E) + Send + 'static,
    R: Send + 'static,
{
    let (sender, receiver) = oneshot::channel();
    let remote = Box::new(|vs: &mut dyn Iterator<Item = (&Path, &V)>| {
        let (r, e) = func(vs);
        let _ = sender.send(r);
        e
    });
    (remote, receiver)
}
