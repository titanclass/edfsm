// Declare our state, commands and events

use edfsm::{impl_fsm, Fsm};

struct Idle;
struct Running;
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

struct Started;
struct Stopped;
enum Event {
    Started(Started),
    Stopped(Stopped),
}

// Declare an object to handle effects as we step through the FSM

struct EffectHandlers {
    started: u32,
    stopped: u32,
    transitioned_stopped_to_started: u32,
    transitioned_started_to_stopped: u32,
}

impl EffectHandlers {
    pub fn start_something(&mut self) {
        self.started += 1;
    }

    pub fn stop_something(&mut self) {
        self.stopped += 1;
    }

    pub fn transitioned_started_to_stopped(&mut self) {
        self.transitioned_started_to_stopped += 1;
    }

    pub fn transitioned_stopped_to_started(&mut self) {
        self.transitioned_stopped_to_started += 1;
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

impl MyFsm {
    fn for_running_stop_stopped(
        _s: &Running,
        _c: &Stop,
        se: &mut EffectHandlers,
    ) -> Option<Stopped> {
        se.stop_something();
        Some(Stopped)
    }

    fn for_idle_start_started(_s: &Idle, _c: &Start, se: &mut EffectHandlers) -> Option<Started> {
        se.start_something();
        Some(Started)
    }

    fn for_running_stopped_idle(_s: &Running, _e: &Stopped) -> Option<Idle> {
        Some(Idle)
    }

    fn for_idle_started_running(_s: &Idle, _e: &Started) -> Option<Running> {
        Some(Running)
    }
}

#[test]
fn main() {
    // Initialize our effect handlers

    let mut se = EffectHandlers {
        started: 0,
        stopped: 0,
        transitioned_stopped_to_started: 0,
        transitioned_started_to_stopped: 0,
    };

    // Finally, test the FSM by stepping through various states

    let (e, t) = MyFsm::step(&State::Idle(Idle), &Command::Start(Start), &mut se);
    assert!(matches!(e, Some(Event::Started(Started))));
    assert!(matches!(t, Some(State::Running(Running))));
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 0);
    assert_eq!(se.transitioned_started_to_stopped, 0);
    assert_eq!(se.transitioned_stopped_to_started, 1);

    let (e, t) = MyFsm::step(&State::Running(Running), &Command::Start(Start), &mut se);
    assert!(e.is_none());
    assert!(t.is_none());
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 0);
    assert_eq!(se.transitioned_started_to_stopped, 0);
    assert_eq!(se.transitioned_stopped_to_started, 1);

    let (e, t) = MyFsm::step(&State::Running(Running), &Command::Stop(Stop), &mut se);
    assert!(matches!(e, Some(Event::Stopped(Stopped))));
    assert!(matches!(t, Some(State::Idle(Idle))));
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 1);
    assert_eq!(se.transitioned_started_to_stopped, 1);
    assert_eq!(se.transitioned_stopped_to_started, 1);

    let (e, t) = MyFsm::step(&&State::Idle(Idle), &Command::Stop(Stop), &mut se);
    assert!(e.is_none());
    assert!(t.is_none());
    assert_eq!(se.started, 1);
    assert_eq!(se.stopped, 1);
    assert_eq!(se.transitioned_started_to_stopped, 1);
    assert_eq!(se.transitioned_stopped_to_started, 1);
}
