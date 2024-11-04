use crate::error::Result;
use core::{future::Future, marker::PhantomData};
use futures_util::{Stream, StreamExt};
#[cfg(feature = "tokio")]
use tokio::sync::mpsc::Sender;

/// A trait to intercept messages in a `Machine` for logging and outbound communication.
/// Adapters can be combined and this is the basis of a wiring scheme for machines.  
pub trait Adapter {
    type Item;

    /// Forward the given item to an asynchronous consumer, possibly converting the type
    /// or possibly dropping the item if it cannot be converted.
    fn notify(&mut self, a: Self::Item) -> impl Future<Output = Result<()>>
    where
        Self::Item: Clone + 'static;

    /// Consume the given asyn `Stream`, passing each item to this adapter.
    /// This adapter is then dropped.
    fn notify_all<S>(mut self, mut stream: S) -> impl Future<Output = Result<()>>
    where
        Self: Sized,
        S: Stream<Item = Self::Item> + Unpin,
        Self::Item: Clone + 'static,
    {
        async move {
            while let Some(a) = stream.next().await {
                self.notify(a).await?;
            }
            Ok(())
        }
    }

    /// Combine two adapters. The notify call is delegated to both adapters.
    fn merge<T>(self, other: T) -> impl Adapter<Item = Self::Item>
    where
        T: Adapter<Item = Self::Item>,
        Self: Sized,
    {
        Merge {
            first: self,
            next: other,
        }
    }

    /// Create an adapter that maps items with an optional function.
    /// `Some` values are passed on, analogous to `Iterator::filter_map`.
    fn adapt_filter_map<A>(self, func: impl Fn(A) -> Option<Self::Item>) -> impl Adapter<Item = A>
    where
        Self: Sized,
        Self::Item: Clone + 'static,
    {
        FilterMap {
            func,
            inner: self,
            marker: PhantomData,
        }
    }

    /// Create an adapter that maps each item with a function.
    fn adapt_map<A>(self, func: impl Fn(A) -> Self::Item) -> impl Adapter<Item = A>
    where
        Self: Sized,
        Self::Item: Clone + 'static,
    {
        self.adapt_filter_map(move |a| Some(func(a)))
    }

    /// Create an adapter that converts each item from another type.
    fn adapt_from<A>(self) -> impl Adapter<Item = A>
    where
        Self: Sized,
        Self::Item: Clone + 'static,
        A: Into<Self::Item>,
    {
        self.adapt_filter_map::<A>(move |a| Some(a.into()))
    }

    /// Create an adapter that fallibly converts each item from another type.
    fn adapt_try_from<A>(self) -> impl Adapter<Item = A>
    where
        Self: Sized,
        Self::Item: Clone + 'static,
        A: TryInto<Self::Item>,
    {
        self.adapt_filter_map::<A>(move |a| a.try_into().ok())
    }
}

/// A  placeholder `Adapter` that discards all items and never notifies.
#[derive(Debug)]
pub struct Discard<Event>(PhantomData<Event>);

impl<A> Default for Discard<A> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A> Adapter for Discard<A> {
    type Item = A;

    /// Discard the item
    async fn notify(&mut self, _e: Self::Item) -> Result<()> {
        Ok(())
    }

    /// Replace this placeholder with the given adapter.
    fn merge<N>(self, other: N) -> impl Adapter<Item = Self::Item>
    where
        N: Adapter<Item = Self::Item>,
    {
        other
    }
}

/// An `Adapter` that bifucates notifications.  This contains two downstream adapters.
#[derive(Debug)]
pub struct Merge<S, T> {
    first: S,
    next: T,
}

impl<E, S, T> Adapter for Merge<S, T>
where
    S: Adapter<Item = E>,
    T: Adapter<Item = E>,
{
    type Item = E;

    async fn notify(&mut self, a: Self::Item) -> Result<()>
    where
        Self::Item: Clone + 'static,
    {
        self.first.notify(a.clone()).await?;
        self.next.notify(a).await
    }
}

/// An `Adapter` that passes each item through an optional function
/// and passes the `Some` values on.
#[derive(Debug)]
pub struct FilterMap<A, F, G> {
    func: F,
    inner: G,
    marker: PhantomData<A>,
}

impl<F, G, A, B> Adapter for FilterMap<A, F, G>
where
    F: Fn(A) -> Option<B>,
    B: Clone + 'static,
    G: Adapter<Item = B>,
{
    type Item = A;

    async fn notify(&mut self, a: Self::Item) -> Result<()>
    where
        Self::Item: Clone + 'static,
    {
        if let Some(b) = (self.func)(a) {
            self.inner.notify(b).await?;
        }
        Ok(())
    }
}

/// A `Adapter` that forwards messages to an mpsc channel.
#[derive(Debug)]
pub struct AdaptChannel<A> {
    sender: Sender<A>,
}

impl<A> AdaptChannel<A> {
    /// Create and `Adapter` that passes on items to a channel.
    pub fn new(sender: Sender<A>) -> Self {
        Self { sender }
    }
}

impl<A> Adapter for AdaptChannel<A> {
    type Item = A;

    async fn notify(&mut self, a: Self::Item) -> Result<()> {
        self.sender.send(a).await?;
        Ok(())
    }
}
