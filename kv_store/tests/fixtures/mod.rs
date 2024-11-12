use edfsm::{Change, Drain, Fsm, Init, Terminating};
use serde::{Deserialize, Serialize};

pub struct Counter;

#[derive(Clone, Debug)]
pub enum Command {
    Print,
    Assert(i32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Tick,
    Reset,
}

#[derive(Clone, Debug)]
pub enum Output {
    Tock,
}

#[derive(Debug, Default)]
pub struct State {
    pub count: i32,
}

impl Fsm for Counter {
    type S = State;
    type C = Command;
    type E = Event;
    type SE = OutputBuffer<Output>;

    fn for_command(s: &Self::S, c: Self::C, _se: &mut Self::SE) -> Option<Self::E> {
        match c {
            Command::Print => println!("count = {}", s.count),
            Command::Assert(count) => assert_eq!(count, s.count),
        }
        None
    }

    fn on_event(s: &mut Self::S, e: &Self::E) -> Option<edfsm::Change> {
        match e {
            Event::Tick => {
                s.count += 1;
                Some(Change::Updated)
            }
            Event::Reset => {
                if s.count == 0 {
                    None
                } else {
                    s.count = 0;
                    Some(Change::Updated)
                }
            }
        }
    }

    fn on_change(s: &Self::S, _e: &Self::E, se: &mut Self::SE, _change: edfsm::Change) {
        if s.count % 10 == 0 {
            se.push(Output::Tock);
        }
    }
}

#[derive(Debug)]
pub struct OutputBuffer<A>(pub std::vec::Vec<A>);

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

impl Terminating for Event {
    fn terminating(&self) -> bool {
        matches!(self, Event::Reset)
    }
}
