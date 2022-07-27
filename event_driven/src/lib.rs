//! Event driven Finite State Machines process commands (possibly created by other
//! events), performing some side effect, and emitting events.
//! Commands are processed against a provided state. Events can be applied to states
//! to yield new states.
//!
//! For more background on [Event-driven Finite State Machines](http://christopherhunt-software.blogspot.com/2021/02/event-driven-finite-state-machines.html).

#![no_std]

pub use event_driven_macros::impl_fsm;

/// Describes the behavior of a Finite State Machine (FSM) that can receive commands and produce
/// events. Along the way, effects can be performed given the receipt of a command.
/// State can be reconsituted by replaying events.
///
/// Note that effects are represented by a separate structure so that they can be consolidated,
/// and also to help structure the code. Further, it is possible to have multiple implementations
/// of effects e.g. different ones when testing.
///
/// Effects are also synchronous. If an effect handler must communicate, say, with a task in a
/// non-blocking fashion, then the state machine should represent this intermediate state.
///
/// The generic types refer to:
/// S  = State          - the state of your FSM
/// C  = Command        - the command(s) that are able to be processed on your FSM
/// E  = Event          - the event(s) that are emitted having performed a command
/// SE = State Effect   - the effect handler
pub trait Fsm<S, C, E, SE> {
    /// Given a state and command, optionally emit an event. Can perform side
    /// effects along the way. This function is generally only called from the
    /// `run` function.
    fn for_command(s: &S, c: &C, se: &mut SE) -> Option<E>;

    /// Given a state and event, produce a transition, which could transition to
    /// the next state. No side effects are to be performed. Can be used to replay
    /// events to attain a new state i.e. the major function of event sourcing.
    fn for_event(s: &S, e: &E) -> Option<S>;

    /// Optional logic for when transitioning into a new state.
    fn on_transition(_old_s: &S, _new_s: &S, _c: &C, _e: &E, _se: &mut SE) {}

    /// This is the main entry point to the event driven FSM.
    /// Runs the state machine for a command, optionally performing effects,
    /// producing an event and transitioning to a new state. Also
    /// applies any "Entry/" or "Exit/" processing when arriving
    /// at a new state.
    fn step(s: &S, c: &C, se: &mut SE) -> (Option<E>, Option<S>) {
        let e = Self::for_command(s, c, se);
        let t = if let Some(e) = &e {
            let t = Self::for_event(s, e);
            if let Some(new_s) = &t {
                Self::on_transition(s, new_s, c, e, se);
            };
            t
        } else {
            None
        };
        (e, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step() {
        // Declare our state, commands and events

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

        impl Fsm<State, Command, Event, EffectHandlers> for MyFsm {
            fn for_command(s: &State, c: &Command, se: &mut EffectHandlers) -> Option<Event> {
                match (s, c) {
                    (State::Running(s), Command::Stop(c)) => {
                        Self::for_running_stop_stopped(s, c, se).map(|r| Event::Stopped(r))
                    }
                    (State::Idle(s), Command::Start(c)) => {
                        Self::for_idle_start_started(s, c, se).map(|r| Event::Started(r))
                    }
                    _ => None,
                }
            }

            fn for_event(s: &State, e: &Event) -> Option<State> {
                match (s, e) {
                    (State::Running(s), Event::Stopped(e)) => {
                        Self::for_running_stopped_idle(s, e).map(|r| State::Idle(r))
                    }
                    (State::Idle(s), Event::Started(e)) => {
                        Self::for_idle_started_running(s, e).map(|r| State::Running(r))
                    }
                    _ => None,
                }
            }

            // Let's implement this optional function to show how entry/exit
            // processing can be achieved, and also confirm that our FSM is
            // calling it.
            fn on_transition(
                old_s: &State,
                new_s: &State,
                c: &Command,
                e: &Event,
                se: &mut EffectHandlers,
            ) {
                match (old_s, new_s) {
                    (State::Running(_), State::Idle(_)) => match (c, e) {
                        (Command::Stop(_), Event::Stopped(_)) => {
                            se.transitioned_started_to_stopped()
                        }
                        _ => {
                            panic!("Unexpected transition");
                        }
                    },
                    (State::Idle(_), State::Running(_)) => match (c, e) {
                        (Command::Start(_), Event::Started(_)) => {
                            se.transitioned_stopped_to_started()
                        }
                        _ => {
                            panic!("Unexpected transition");
                        }
                    },
                    _ => {
                        panic!("Unexpected transition");
                    }
                }
            }
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

            fn for_idle_start_started(
                _s: &Idle,
                _c: &Start,
                se: &mut EffectHandlers,
            ) -> Option<Started> {
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

        // Initialize our effect handlers

        let mut se = EffectHandlers {
            started: 0,
            stopped: 0,
            transitioned_stopped_to_started: 0,
            transitioned_started_to_stopped: 0,
        };

        // Finally, test the FSM by stepping through various states

        let (e, t) = MyFsm::step(&State::Idle(Idle), &Command::Start(Start), &mut se);
        assert_eq!(e, Some(Event::Started(Started)));
        assert_eq!(t, Some(State::Running(Running)));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_started_to_stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Running(Running), &Command::Start(Start), &mut se);
        assert_eq!(e, None);
        assert_eq!(t, None);
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_started_to_stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Running(Running), &Command::Stop(Stop), &mut se);
        assert_eq!(e, Some(Event::Stopped(Stopped)));
        assert_eq!(t, Some(State::Idle(Idle)));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_started_to_stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&&State::Idle(Idle), &Command::Stop(Stop), &mut se);
        assert_eq!(e, None);
        assert_eq!(t, None);
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_started_to_stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);
    }
}
