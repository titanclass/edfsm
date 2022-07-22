// Declare our state, commands and events

use edfsm::{impl_fsm, Fsm, Transition};

#[derive(Debug, PartialEq)]
enum State {
    Started,
    Stopped,
}

enum Command {
    Start,
    Stop,
}

#[derive(Debug, PartialEq)]
enum Event {
    Started,
    Stopped,
}

// Declare an object to handle effects as we step through the FSM

struct EffectHandlers {
    started: u32,
    stopped: u32,
    transitioned_from_stopped: u32,
    transitioned_from_started: u32,
}

impl EffectHandlers {
    pub fn start_something(&mut self) {
        self.started += 1;
    }

    pub fn stop_something(&mut self) {
        self.stopped += 1;
    }

    pub fn transitioned_from_started(&mut self) {
        self.transitioned_from_started += 1;
    }

    pub fn transitioned_from_stopped(&mut self) {
        self.transitioned_from_stopped += 1;
    }
}

// Declare the FSM itself

struct MyFsm {}

#[impl_fsm]
impl Fsm<State, Command, Event, EffectHandlers> for MyFsm {
    state!(State::Started / exit);
    state!(State::Stopped / exit);

    transition!(State::Stopped => Command::Start => Event::Started => State::Started);
    transition!(State::Started => Command::Stop  => Event::Stopped => State::Stopped);
}

#[test]
fn main() {
    // Initialize our effect handlers

    let mut se = EffectHandlers {
        started: 0,
        stopped: 0,
        transitioned_from_stopped: 0,
        transitioned_from_started: 0,
    };

    // Finally, test the FSM by stepping through various states

    let (e, t) = MyFsm::step(&State::Stopped, &Command::Start, &mut se);
    assert_eq!(e, Some(Event::Started));
    assert_eq!(t, Transition::Next(State::Started));
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 0);
    assert_eq!(se.transitioned_from_started, 0);
    assert_eq!(se.transitioned_from_stopped, 1);

    let (e, t) = MyFsm::step(&State::Started, &Command::Start, &mut se);
    assert_eq!(e, None);
    assert_eq!(t, Transition::Same);
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 0);
    assert_eq!(se.transitioned_from_started, 0);
    assert_eq!(se.transitioned_from_stopped, 1);

    let (e, t) = MyFsm::step(&State::Started, &Command::Stop, &mut se);
    assert_eq!(e, Some(Event::Stopped));
    assert_eq!(t, Transition::Next(State::Stopped));
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 1);
    assert_eq!(se.transitioned_from_started, 1);
    assert_eq!(se.transitioned_from_stopped, 1);

    let (e, t) = MyFsm::step(&&State::Stopped, &Command::Stop, &mut se);
    assert_eq!(e, None);
    assert_eq!(t, Transition::Same);
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 1);
    assert_eq!(se.transitioned_from_started, 1);
    assert_eq!(se.transitioned_from_stopped, 1);
}
