//! Event driven Finite State Machines process commands (possibly created by other
//! events), performing some side effect, and emitting events.
//! Commands are processed against a provided state. Events can be applied to states
//! to yield new states.
//!
//! For more background on [Event-driven Finite State Machines](http://christopherhunt-software.blogspot.com/2021/02/event-driven-finite-state-machines.html).

#![no_std]

use core::future::Future;

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
/// non-blocking fashion, then the state machine should represent this intermediate state. For
/// example, a channel could be used to communicate with such a task with `try_send` being used
/// and then causing a state transition in relation to that result. While this approach adds
/// steps to a state machine, it does allow them to remain responsive to receiving more
/// commands.
pub trait Fsm {
    /// The state managed by the FSM
    type S: Send;
    /// The command(s) that are able to be processed by the FSM
    type C: Send;
    /// The event emitted having performed a command
    type E: Send;
    /// The side effect handler
    type SE: Send;

    /// Given a state and command, optionally emit an event. Can perform async side
    /// effects along the way. This function is generally only called from the
    /// `run` function.
    fn for_command(
        s: &Self::S,
        c: Self::C,
        se: &mut Self::SE,
    ) -> impl Future<Output = Option<Self::E>> + Send;

    /// Given a state and event, modify state, which could indicate transition to
    /// the next state. No side effects are to be performed. Can be used to replay
    /// events to attain a new state i.e. the major function of event sourcing.
    fn on_event(s: &mut Self::S, e: &Self::E) -> bool;

    /// Optional effect on entering a state i.e. transitioning in to state `S` from
    /// another.
    fn on_entry(_s: &Self::S, _se: &mut Self::SE) -> impl Future<Output = ()> + Send;

    /// This is the main entry point to the event driven FSM.
    /// Runs the state machine for a command, optionally performing effects,
    /// possibly producing an event and possibly transitioning to a new state. Also
    /// applies any "Entry/" processing when arriving at a new state.
    fn step(
        s: &mut Self::S,
        c: Self::C,
        se: &mut Self::SE,
    ) -> impl Future<Output = Option<Self::E>> + Send {
        async {
            let e = Self::for_command(s, c, se).await;
            if let Some(e) = &e {
                let t = Self::on_event(s, e);
                if t {
                    Self::on_entry(s, se).await;
                };
            };
            e
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_log::test;

    #[test(tokio::test)]
    async fn test_step() {
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

            async fn for_command(s: &State, c: Command, se: &mut EffectHandlers) -> Option<Event> {
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

            fn on_event(mut s: &mut State, e: &Event) -> bool {
                let new_s = match (&mut s, e) {
                    (State::Running(s), Event::Stopped(e)) => {
                        Self::on_running_stopped(s, e).map(State::Idle)
                    }
                    (State::Idle(s), Event::Started(e)) => {
                        Self::on_idle_started(s, e).map(State::Running)
                    }
                    _ => None,
                };
                if let Some(new_s) = new_s {
                    *s = new_s;
                    true
                } else {
                    false
                }
            }

            // Let's implement this optional function to show how entry/exit
            // processing can be achieved, and also confirm that our FSM is
            // calling it.
            async fn on_entry(new_s: &State, se: &mut EffectHandlers) {
                if let State::Running(s) = new_s {
                    Self::on_entry_running(s, se)
                }
            }
        }

        impl MyFsm {
            fn on_entry_running(_to_s: &Running, se: &mut EffectHandlers) {
                se.enter_running()
            }

            fn for_running_stop(
                _s: &Running,
                _c: Stop,
                se: &mut EffectHandlers,
            ) -> Option<Stopped> {
                se.stop_something();
                Some(Stopped)
            }

            fn on_running_stopped(_s: &Running, _e: &Stopped) -> Option<Idle> {
                Some(Idle)
            }

            fn for_idle_start(_s: &Idle, _c: Start, se: &mut EffectHandlers) -> Option<Started> {
                se.start_something();
                Some(Started)
            }

            fn on_idle_started(_s: &Idle, _e: &Started) -> Option<Running> {
                Some(Running)
            }
        }

        // Initialize our effect handlers

        let mut se = EffectHandlers {
            started: 0,
            stopped: 0,
            transitioned_stopped_to_started: 0,
        };

        // Finally, test the FSM by stepping through various states

        let e = MyFsm::step(&mut State::Idle(Idle), Command::Start(Start), &mut se).await;
        assert!(matches!(e, Some(Event::Started(Started))));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(&mut State::Running(Running), Command::Start(Start), &mut se).await;
        assert!(e.is_none());
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(&mut State::Running(Running), Command::Stop(Stop), &mut se).await;
        assert!(matches!(e, Some(Event::Stopped(Stopped))));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let e = MyFsm::step(&mut State::Idle(Idle), Command::Stop(Stop), &mut se).await;
        assert!(e.is_none());
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);
    }
}
