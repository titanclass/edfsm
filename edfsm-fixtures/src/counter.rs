use edfsm::{Change, Fsm};
use serde::{Deserialize, Serialize};
use streambed_machine::CompactionKey;

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

impl CompactionKey for Event {
    fn compaction_key(&self) -> u64 {
        0
    }
}

#[derive(Clone, Debug)]
pub enum Output {
    Tock,
}

#[derive(Debug, Default)]
pub struct State {
    count: i32,
}

impl Fsm for Counter {
    type S = State;
    type C = Command;
    type E = Event;
    type SE = Vec<Output>;

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
