use crate::{Keyed, Path, Query, RespondMany, RespondOne};
use alloc::boxed::Box;
use core::ops::Bound;
use edfsm::Input;
use machine::{adapter::Adapter, error::Result};
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
    T: Adapter<Item = Input<Query<V>, Keyed<E>>>,
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
        let (sender, receiver) = oneshot::channel::<R>();
        let q = Query::Get(path, respond_one(func, sender));
        self.0.notify(Input::Command(q)).await?;
        Ok(receiver.await?)
    }

    /// Get the entries whose path starts with the given path,
    /// including the entry for the path itself.
    /// Apply `func` to these and return the result.
    pub async fn get_tree<F, R>(&mut self, path: Path, func: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.dispatch_many_valued(|r| Query::GetTree(path, r), func)
            .await
    }

    /// Get the entries in the given range
    /// Apply `func` to these and return the result.
    pub async fn get_range<F, R>(&mut self, range: (Bound<Path>, Bound<Path>), func: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.dispatch_many_valued(|r| Query::GetRange(range, r), func)
            .await
    }

    /// Get all the entries
    /// Apply `func` to these and return the result.
    pub async fn get_all<F, R>(&mut self, func: F) -> Result<R>
    where
        F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> R + Send + 'static,
        R: Send + 'static,
    {
        self.dispatch_many_valued(Query::GetAll, func).await
    }

    async fn dispatch_many_valued<Q, F, R>(&mut self, query: Q, func: F) -> Result<R>
    where
        Q: FnOnce(RespondMany<V>) -> Query<V>,
        F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel::<R>();
        let q = query(respond_many(func, sender));
        self.0.notify(Input::Command(q)).await?;
        Ok(receiver.await?)
    }
}

fn respond_one<F, V, R>(func: F, sender: oneshot::Sender<R>) -> RespondOne<V>
where
    F: FnOnce(Option<&V>) -> R + Send + 'static,
    R: Send + 'static,
{
    Box::new(|v| {
        let _ = sender.send(func(v));
    })
}

fn respond_many<F, V, R>(func: F, sender: oneshot::Sender<R>) -> RespondMany<V>
where
    F: FnOnce(&mut dyn Iterator<Item = (&Path, &V)>) -> R + Send + 'static,
    R: Send + 'static,
{
    Box::new(|vs| {
        let _ = sender.send(func(vs));
    })
}
