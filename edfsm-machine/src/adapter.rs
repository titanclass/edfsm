use crate::error::Result;
use core::{future::Future, marker::PhantomData};

/// A trait to intercept messages in a `Machine` for logging and outbound communication.
///
/// Adapters can be combined and this is the basis of a wiring scheme for machines.  
/// For the absence of doubt, all `Adapter`s are `Send` meaning they can be part of the
/// state of a task in a multithreaded environment.
pub trait Adapter: Send {
    type Item;

    /// Forward the given item to an asynchronous consumer, possibly converting the type
    /// or possibly dropping the item if it cannot be converted.
    fn notify(&mut self, a: Self::Item) -> impl Future<Output = ()> + Send
    where
        Self::Item: 'static;

    /// Clone the referenced item and then forward it to an asynchonous consumer.
    /// The clone operation can is avoid in the `Placeholder` implementation.
    fn clone_notify(&mut self, a: &Self::Item) -> impl Future<Output = ()> + Send
    where
        Self::Item: Clone + 'static,
    {
        self.notify(a.clone())
    }

    /// Combine this with another adapter. The notify call is delegated to both adapters.
    fn merge<T>(self, other: T) -> impl Adapter<Item = Self::Item>
    where
        T: Adapter<Item = Self::Item>,
        Self: Sized + Send,
        Self::Item: Send + Clone,
    {
        Merge {
            first: self,
            next: other,
        }
    }

    /// Create an adapter that maps items with an optional function.
    /// `Some` values are passed on, analogous to `Iterator::filter_map`.
    fn with_filter_map<A>(
        self,
        func: impl Fn(A) -> Option<Self::Item> + Send,
    ) -> impl Adapter<Item = A>
    where
        Self: Sized + Send,
        Self::Item: Send + 'static,
        A: Send,
    {
        FilterMap {
            func,
            inner: self,
            marker: PhantomData,
        }
    }

    /// Create an adapter that maps each item with a function.
    fn with_map<A>(self, func: impl Fn(A) -> Self::Item + Send) -> impl Adapter<Item = A>
    where
        Self: Sized + Send,
        Self::Item: Send + 'static,
        A: Send,
    {
        self.with_filter_map(move |a| Some(func(a)))
    }

    /// Create an adapter that converts each item from another type.
    /// This relies on an `Into` implementation for the conversion.
    fn adapt<A>(self) -> impl Adapter<Item = A>
    where
        Self: Sized + Send,
        Self::Item: Send + 'static,
        A: Into<Self::Item> + Send,
    {
        self.with_filter_map::<A>(move |a| Some(a.into()))
    }

    /// Create an adapter that fallibly converts each item from another type.
    /// This relies on an `TryInto` implementation for the conversion.
    fn adapt_fallible<A>(self) -> impl Adapter<Item = A>
    where
        Self: Sized + Send,
        Self::Item: Send + 'static,
        A: TryInto<Self::Item> + Send,
    {
        self.with_filter_map::<A>(move |a| a.try_into().ok())
    }
}

/// A  placeholder for an `Adapter` and/or `Feed`.
///
/// As an `Adapter` this discards all items. As a `Feed` it provides no items.
#[derive(Debug)]
pub struct Placeholder<Event>(PhantomData<Event>);

impl<A> Default for Placeholder<A> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A> Adapter for Placeholder<A>
where
    A: Send,
{
    type Item = A;

    /// Discard the item
    async fn notify(&mut self, _e: Self::Item) {}

    /// Ignore the reference and avoid the clone.
    #[allow(clippy::manual_async_fn)]
    fn clone_notify(&mut self, _a: &Self::Item) -> impl Future<Output = ()> + Send {
        async {}
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

impl<A, S, T> Adapter for Merge<S, T>
where
    S: Adapter<Item = A> + Send,
    T: Adapter<Item = A> + Send,
    A: Send + Clone,
{
    type Item = A;

    async fn notify(&mut self, a: Self::Item)
    where
        Self::Item: 'static,
    {
        self.first.notify(a.clone()).await;
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
    B: Send + 'static,
    G: Adapter<Item = B> + Send,
    A: Send,
{
    type Item = A;

    async fn notify(&mut self, a: Self::Item)
    where
        Self::Item: 'static,
    {
        if let Some(b) = (self.func)(a) {
            self.inner.notify(b).await;
        }
    }
}

/// Implement `Adapter` for a vector
#[cfg(feature = "std")]
impl<A> Adapter for std::vec::Vec<A>
where
    A: Send,
{
    type Item = A;

    async fn notify(&mut self, a: Self::Item)
    where
        Self::Item: 'static,
    {
        self.push(a);
    }
}

/// Implement `Feed` for a vector
#[cfg(feature = "std")]
impl<A> Feed for std::vec::Vec<A>
where
    A: Clone + Send + Sync + 'static,
{
    type Item = A;

    async fn feed(&self, output: &mut impl Adapter<Item = Self::Item>) -> Result<()> {
        for a in self.iter().cloned() {
            output.notify(a).await;
        }
        Ok(())
    }
}

/// Implementations of  `Adapter` for tokio channels.
#[cfg(feature = "tokio")]
pub mod adapt_tokio {
    use crate::adapter::Adapter;
    use tokio::sync::{broadcast, mpsc};

    impl<A> Adapter for mpsc::Sender<A>
    where
        A: Send,
    {
        type Item = A;

        async fn notify(&mut self, a: Self::Item) {
            let _ = self.send(a).await;
        }
    }

    impl<A> Adapter for broadcast::Sender<A>
    where
        A: Send,
    {
        type Item = A;

        async fn notify(&mut self, a: Self::Item) {
            let _ = self.send(a);
        }
    }
}

/// A source of messages that can `feed` an `Adapter`.
pub trait Feed {
    type Item;

    /// Send a stream of messages into an adapter.
    fn feed(
        &self,
        output: &mut impl Adapter<Item = Self::Item>,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl<A> Feed for Placeholder<A>
where
    A: Send,
    Self: Sync,
{
    type Item = A;

    async fn feed(&self, _: &mut impl Adapter<Item = Self::Item>) -> Result<()> {
        Ok(())
    }
}

/// Implementations of `Adapter` for streambed
#[cfg(feature = "streambed")]
mod adapt_streambed {
    use crate::{
        adapter::{Adapter, Feed},
        error::Result,
    };
    use futures_util::StreamExt;
    use streambed_codec::{Codec, CommitLog, LogAdapter};

    impl<L, C, A> Feed for LogAdapter<L, C, A>
    where
        C: Codec<A> + Sync + Send,
        L: CommitLog + Sync + Send,
        A: Send + Sync + 'static,
    {
        type Item = A;

        async fn feed(&self, output: &mut impl Adapter<Item = Self::Item>) -> Result<()> {
            let mut s = self.history().await;
            while let Some(a) = s.next().await {
                output.notify(a).await;
            }
            Ok(())
        }
    }

    impl<L, C, A> Adapter for LogAdapter<L, C, A>
    where
        C: Codec<A> + Sync + Send,
        L: CommitLog + Sync + Send,
        A: Sync + Send,
    {
        type Item = A;

        async fn notify(&mut self, a: Self::Item)
        where
            Self::Item: 'static,
        {
            let _ = self.produce(a).await;
        }
    }
}
