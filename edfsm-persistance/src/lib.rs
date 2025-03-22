use core::{future::Future, marker::PhantomData};
use edfsm_machine::adapter::{Adapter, Feed};
use edfsm_machine::error::Result;

#[derive(Debug)]
pub struct BackingStore<A> {
    marker: PhantomData<A>,
}

impl<A> Feed for BackingStore<A> {
    type Item = A;

    fn feed(
        &self,
        _item: &mut impl Adapter<Item = Self::Item>,
    ) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }
}

impl<A> Adapter for BackingStore<A>
where
    A: Send,
{
    type Item = A;

    fn notify(&mut self, _item: Self::Item) -> impl Future<Output = ()> + Send
    where
        Self::Item: 'static,
    {
        async {}
    }
}
