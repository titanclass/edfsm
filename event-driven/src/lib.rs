#![doc = include_str!("../../README.md")]
#![no_std]

pub use event_driven_macros::impl_fsm;

/// A type of input - commands or events.
pub enum Input<C, E> {
    Command(C),
    Event(E),
}

/// Describes a type of state change that `on_event` can perform.
pub enum Change {
    Transitioned,
    Updated,
}

/// Runs the state machine for a command or event, optionally performing effects,
/// events, or receive events. These types of FSM can be broadly described as "Mealy" and "Moore" machines
/// respectively. Along the way, effects can be performed given the receipt of a command or the application
/// of an event. State can be reconsituted by replaying events.
///
/// Note that effects are represented by a separate structure so that they can be consolidated,
/// and also to help structure the code. Further, it is possible to have multiple implementations
/// of effects e.g. different ones when testing.
///
/// Effects are also synchronous. If an effect handler must communicate, say, with a task in a
/// non-blocking fashion, then the state machine should represent this intermediate state. For
/// example, a channel could be used to communicate with such a task with `try_send` being used
/// and then causing a state transition in relation to that result. While this approach adds
/// steps to a state machine, it does allow them to remain responsive to receiving more
/// commands.
pub trait Fsm {
    /// The state managed by the FSM
    type S;
    /// The command(s) that are able to be processed by the FSM
    type C;
    /// The event emitted having performed a command
    type E;
    /// The side effect handler
    type SE;

    /// Given a state and command, optionally emit an event if it applies. Can perform side
    /// effects. This function is generally only called from the
    /// `step` function.
    fn for_command(s: &Self::S, c: Self::C, se: &mut Self::SE) -> Option<Self::E>;

    /// Given a state and event, modify state, which could indicate transition to
    /// the next state. No side effects are to be performed. Can be used to replay
    /// events to attain a new state i.e. the major function of event sourcing.
    /// Returns some enumeration of the `Change` type if there is a state transition.
    fn on_event(s: &mut Self::S, e: &Self::E) -> Option<Change>;

    /// Given a state and event having been applied then handle any potential change
    /// and optionally perform side effects.
    /// This function is generally only called from the `step` function.
    fn on_change(s: &Self::S, e: &Self::E, se: &mut Self::SE, change: Change);

    /// This is the common entry point to the event driven FSM.
    /// Runs the state machine for a command input, optionally performing effects,
    /// possibly producing an event and possibly transitioning to a new state. Also
    /// applies any "Entry/" processing when arriving at a new state, and a change
    /// handler if there is a state change.
    fn step(s: &mut Self::S, i: Input<Self::C, Self::E>, se: &mut Self::SE) -> Option<Self::E> {
        let e = match i {
            Input::Command(c) => Self::for_command(s, c, se),
            Input::Event(e) => Some(e),
        };
        if let Some(e) = e {
            let r = Self::on_event(s, &e);
            if let Some(c) = r {
                Self::on_change(s, &e, se, c);
                Some(e)
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step() {
        // Declare our state, commands and events

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
        }

        impl EffectHandlers {
            pub fn start_something(&mut self) {
                self.started += 1;
            }

            pub fn stop_something(&mut self) {
                self.stopped += 1;
            }

            pub fn enter_running(&mut self) {
                self.transitioned_stopped_to_started += 1;
            }
        }

        // Declare the FSM itself

        struct MyFsm;

        impl Fsm for MyFsm {
            type S = State;
            type C = Command;
            type E = Event;
            type SE = EffectHandlers;

            fn for_command(s: &State, c: Command, se: &mut EffectHandlers) -> Option<Event> {
                match (s, c) {
                    (State::Running(s), Command::Stop(c)) => {
                        Self::for_running_stop(s, c, se).map(Event::Stopped)
                    }
                    (State::Idle(s), Command::Start(c)) => {
                        Self::for_idle_start(s, c, se).map(Event::Started)
                    }
                    _ => None,
                }
            }

            fn on_event(mut s: &mut State, e: &Event) -> Option<Change> {
                let r = match (&mut s, e) {
                    (State::Running(s), Event::Stopped(e)) => Self::on_running_stopped(s, e)
                        .map(|new_s| (Change::Transitioned, Some(State::Idle(new_s)))),
                    (State::Idle(s), Event::Started(e)) => Self::on_idle_started(s, e)
                        .map(|new_s| (Change::Transitioned, Some(State::Running(new_s)))),
                    _ => None,
                };
                if let Some((c, new_s)) = r {
                    if let Some(new_s) = new_s {
                        *s = new_s;
                    }
                    Some(c)
                } else {
                    None
                }
            }

            fn on_change(s: &State, e: &Event, se: &mut EffectHandlers, change: Change) {
                if let Change::Transitioned = change {
                    // Let's implement this optional function to show how entry/exit
                    // processing can be achieved, and also confirm that our FSM is
                    // calling it.
                    if let State::Running(s) = s {
                        Self::on_entry_running(s, e, se)
                    }
                }
                match (s, e) {
                    (State::Idle(s), Event::Stopped(e)) => Self::on_idle_stopped(s, e, se),
                    (State::Running(s), Event::Started(e)) => Self::on_running_started(s, e, se),
                    _ => (),
                }
            }
        }

        impl MyFsm {
            fn on_entry_running(_to_s: &Running, _e: &Event, se: &mut EffectHandlers) {
                se.enter_running()
            }

            fn for_running_stop(
                _s: &Running,
                _c: Stop,
                _se: &mut EffectHandlers,
            ) -> Option<Stopped> {
                Some(Stopped)
            }

            fn on_running_started(_s: &Running, _e: &Started, se: &mut EffectHandlers) {
                se.start_something();
            }

            fn on_running_stopped(_s: &Running, _e: &Stopped) -> Option<Idle> {
                Some(Idle)
            }

            fn for_idle_start(_s: &Idle, _c: Start, _se: &mut EffectHandlers) -> Option<Started> {
                Some(Started)
            }

            fn on_idle_started(_s: &Idle, _e: &Started) -> Option<Running> {
                Some(Running)
            }

            fn on_idle_stopped(_s: &Idle, _e: &Stopped, se: &mut EffectHandlers) {
                se.stop_something();
            }
        }

        // Initialize our effect handlers

        let mut se = EffectHandlers {
            started: 0,
            stopped: 0,
            transitioned_stopped_to_started: 0,
        };

        // First, test the FSM by stepping through various states given commands

        let e = MyFsm::step(
            &mut State::Idle(Idle),
            Input::Command(Command::Start(Start)),
            &mut se,
        );
        assert!(matches!(e, Some(Event::Started(Started))));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(
            &mut State::Running(Running),
            Input::Command(Command::Start(Start)),
            &mut se,
        );
        assert!(e.is_none());
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(
            &mut State::Running(Running),
            Input::Command(Command::Stop(Stop)),
            &mut se,
        );
        assert!(matches!(e, Some(Event::Stopped(Stopped))));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(
            &mut State::Idle(Idle),
            Input::Command(Command::Stop(Stop)),
            &mut se,
        );
        assert!(e.is_none());
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        // Reset our effect handlers

        let mut se = EffectHandlers {
            started: 0,
            stopped: 0,
            transitioned_stopped_to_started: 0,
        };

        // Now, test the FSM by stepping through various states given events

        let e = MyFsm::step(
            &mut State::Idle(Idle),
            Input::Event(Event::Started(Started)),
            &mut se,
        );
        assert!(matches!(e, Some(Event::Started(Started))));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(
            &mut State::Running(Running),
            Input::Event(Event::Started(Started)),
            &mut se,
        );
        assert!(e.is_none());
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(
            &mut State::Running(Running),
            Input::Event(Event::Stopped(Stopped)),
            &mut se,
        );
        assert!(matches!(e, Some(Event::Stopped(Stopped))));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(
            &mut State::Idle(Idle),
            Input::Event(Event::Stopped(Stopped)),
            &mut se,
        );
        assert!(e.is_none());
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);
    }
}
