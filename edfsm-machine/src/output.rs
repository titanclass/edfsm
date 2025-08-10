use alloc::vec::Vec;
use edfsm::{Drain, Init};

#[derive(Debug)]
pub struct OutputBuffer<A>(pub Vec<A>);

impl<A> OutputBuffer<A> {
    pub fn push(&mut self, item: A) {
        self.0.push(item);
    }
}

impl<A> Default for OutputBuffer<A> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<A> Drain for OutputBuffer<A>
where
    A: Send,
{
    type Item = A;

    fn drain_all(&mut self) -> impl Iterator<Item = Self::Item> {
        self.0.drain(0..)
    }
}

impl<S, A> Init<S> for OutputBuffer<A> {
    fn init(&mut self, _: &S) {}
}
