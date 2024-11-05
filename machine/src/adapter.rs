use crate::error::Result;
use core::{future::Future, marker::PhantomData};
use futures_util::{Stream, StreamExt};

/// A trait to intercept messages in a `Machine` for logging and outbound communication.
/// Adapters can be combined and this is the basis of a wiring scheme for machines.  
pub trait Adapter {
    type Item;

    /// Forward the given item to an asynchronous consumer, possibly converting the type
    /// or possibly dropping the item if it cannot be converted.
    fn notify(&mut self, a: Self::Item) -> impl Future<Output = Result<()>> + Send
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
    fn merge<T>(self, other: T) -> impl Adapter<Item = Self::Item> + Send
    where
        T: Adapter<Item = Self::Item> + Send,
        Self: Sized + Send,
        Self::Item: Send,
    {
        Merge {
            first: self,
            next: other,
        }
    }

    /// Create an adapter that maps items with an optional function.
    /// `Some` values are passed on, analogous to `Iterator::filter_map`.
    fn adapt_filter_map<A>(
        self,
        func: impl Fn(A) -> Option<Self::Item> + Send,
    ) -> impl Adapter<Item = A> + Send
    where
        Self: Sized + Send,
        Self::Item: Clone + Send + 'static,
        A: Send,
    {
        FilterMap {
            func,
            inner: self,
            marker: PhantomData,
        }
    }

    /// Create an adapter that maps each item with a function.
    fn adapt_map<A>(self, func: impl Fn(A) -> Self::Item + Send) -> impl Adapter<Item = A> + Send
    where
        Self: Sized + Send,
        Self::Item: Clone + Send + 'static,
        A: Send,
    {
        self.adapt_filter_map(move |a| Some(func(a)))
    }

    /// Create an adapter that converts each item from another type.
    fn adapt_from<A>(self) -> impl Adapter<Item = A> + Send
    where
        Self: Sized + Send,
        Self::Item: Clone + Send + 'static,
        A: Into<Self::Item> + Send,
    {
        self.adapt_filter_map::<A>(move |a| Some(a.into()))
    }

    /// Create an adapter that fallibly converts each item from another type.
    fn adapt_try_from<A>(self) -> impl Adapter<Item = A> + Send
    where
        Self: Sized + Send,
        Self::Item: Clone + Send + 'static,
        A: TryInto<Self::Item> + Send,
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

impl<A> Adapter for Discard<A>
where
    A: Send,
{
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
    S: Adapter<Item = E> + Send,
    T: Adapter<Item = E> + Send,
    E: Send,
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
    F: Fn(A) -> Option<B> + Send,
    B: Clone + Send + 'static,
    G: Adapter<Item = B> + Send,
    A: Send,
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

/// Implement `Adapter` for a vector
#[cfg(feature = "std")]
impl<A> Adapter for std::vec::Vec<A>
where
    A: Send,
{
    type Item = A;

    async fn notify(&mut self, a: Self::Item) -> Result<()>
    where
        Self::Item: Clone + 'static,
    {
        Ok(self.push(a))
    }
}

/// Implementations of  `Adapter` for tokio channels.
#[cfg(feature = "tokio")]
pub mod adapt_channel {
    use crate::{adapter::Adapter, error::Result};
    use tokio::sync::{broadcast, mpsc};

    impl<A> Adapter for mpsc::Sender<A>
    where
        A: Send,
    {
        type Item = A;

        async fn notify(&mut self, a: Self::Item) -> Result<()> {
            self.send(a).await?;
            Ok(())
        }
    }

    impl<A> Adapter for broadcast::Sender<A>
    where
        A: Send,
    {
        type Item = A;

        async fn notify(&mut self, a: Self::Item) -> Result<()> {
            self.send(a)?;
            Ok(())
        }
    }
}
