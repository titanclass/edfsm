// Declare our state, commands and events

use edfsm::{impl_fsm, Fsm};

#[derive(Debug, PartialEq)]
struct Idle;

#[derive(Debug, PartialEq)]
struct Running;

#[derive(Debug, PartialEq)]
enum State {
    Idle(Idle),
    Running(Running),
}

struct Start;
struct Stop;

enum Command {
    Start(Start),
    Stop(Stop),
}

#[derive(Debug, PartialEq)]
struct Started;

#[derive(Debug, PartialEq)]
struct Stopped;

#[derive(Debug, PartialEq)]
enum Event {
    Started(Started),
    Stopped(Stopped),
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
    state!(Started / exit);
    state!(Stopped / exit);

    transition!(Idle    => Start => Started => Running);
    transition!(Running => Stop  => Stopped => Idle);
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

    let (e, t) = MyFsm::step(&State::Idle(Idle), &Command::Start(Start), &mut se);
    assert_eq!(e, Some(Event::Started(Started)));
    assert_eq!(t, Some(State::Running(Running)));
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 0);
    assert_eq!(se.transitioned_from_started, 0);
    assert_eq!(se.transitioned_from_stopped, 1);

    let (e, t) = MyFsm::step(&State::Running(Running), &Command::Start(Start), &mut se);
    assert_eq!(e, None);
    assert_eq!(t, None);
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 0);
    assert_eq!(se.transitioned_from_started, 0);
    assert_eq!(se.transitioned_from_stopped, 1);

    let (e, t) = MyFsm::step(&State::Running(Running), &Command::Stop(Stop), &mut se);
    assert_eq!(e, Some(Event::Stopped(Stopped)));
    assert_eq!(t, Some(State::Idle(Idle)));
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 1);
    assert_eq!(se.transitioned_from_started, 1);
    assert_eq!(se.transitioned_from_stopped, 1);

    let (e, t) = MyFsm::step(&&State::Idle(Idle), &Command::Stop(Stop), &mut se);
    assert_eq!(e, None);
    assert_eq!(t, None);
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 1);
    assert_eq!(se.transitioned_from_started, 1);
    assert_eq!(se.transitioned_from_stopped, 1);
}
