//! Event driven Finite State Machines process commands (possibly created by other
//! events), performing some side effect, and emitting events.
//! Commands are processed against a provided state. Events can be applied to states
//! to yield new states.
//!
//! For more background on [Event-driven Finite State Machines](http://christopherhunt-software.blogspot.com/2021/02/event-driven-finite-state-machines.html).

#![cfg_attr(not(any(feature = "async_trait_lib", test)), no_std)]
#![cfg_attr(not(feature = "async_trait_lib"), feature(async_fn_in_trait))]

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
#[cfg_attr(feature = "async_trait_lib", async_trait::async_trait(?Send))]
// FIXME:  Static not require once we remove async-trait lib. Type C must outlive the trait with
// the library.
pub trait Fsm<S, C: 'static, E, SE> {
    /// Given a state and command, optionally emit an event. Can perform side
    /// effects along the way. This function is generally only called from the
    /// `run` function.
    async fn for_command(s: &S, c: C, se: &mut SE) -> Option<E>;

    /// Given a state and event, produce a transition, which could transition to
    /// the next state. No side effects are to be performed. Can be used to replay
    /// events to attain a new state i.e. the major function of event sourcing.
    fn on_event(s: &S, e: &E) -> Option<S>;

    /// Optional effect on exiting a state i.e. transitioning out of state `S` into
    /// another.
    async fn on_exit(_s: &S, _se: &mut SE) {}

    /// Optional effect on entering a state i.e. transitioning in to state `S` from
    /// another.
    async fn on_entry(_s: &S, _se: &mut SE) {}

    /// Determines whether an actual state transition is to occur given two states, 'S'.
    fn is_transitioning(s0: &S, s1: &S) -> bool;

    /// This is the main entry point to the event driven FSM.
    /// Runs the state machine for a command, optionally performing effects,
    /// producing an event and transitioning to a new state. Also
    /// applies any "Entry/" or "Exit/" processing when arriving
    /// at a new state.
    async fn step(s: &S, c: C, se: &mut SE) -> (Option<E>, Option<S>) {
        let e = Self::for_command(s, c, se).await;
        let t = if let Some(e) = &e {
            let t = Self::on_event(s, e);
            if let Some(new_s) = &t {
                if Self::is_transitioning(s, new_s) {
                    Self::on_exit(s, se).await;
                    Self::on_entry(new_s, se).await;
                }
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

    #[tokio::test]
    async fn test_step() {
        // Declare our state, commands and events

        #[derive(Debug)]
        struct Idle;
        #[derive(Debug)]
        struct Running;
        #[derive(Debug)]
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
            pub async fn start_something(&mut self) {
                self.started += 1;
            }

            pub fn stop_something(&mut self) {
                self.stopped += 1;
            }

            pub fn exit_running(&mut self) {
                self.transitioned_started_to_stopped += 1;
            }

            pub fn enter_running(&mut self) {
                self.transitioned_stopped_to_started += 1;
            }
        }

        // Declare the FSM itself

        struct MyFsm {}

        #[cfg_attr(feature = "async_trait_lib", async_trait::async_trait(?Send))]
        impl Fsm<State, Command, Event, EffectHandlers> for MyFsm {
            async fn for_command(s: &State, c: Command, se: &mut EffectHandlers) -> Option<Event> {
                match (s, c) {
                    (State::Running(s), Command::Stop(c)) => {
                        Self::for_running_stop(s, c, se).map(Event::Stopped)
                    }
                    (State::Idle(s), Command::Start(c)) => {
                        Self::for_idle_start(s, c, se).await.map(Event::Started)
                    }
                    _ => None,
                }
            }

            fn on_event(s: &State, e: &Event) -> Option<State> {
                match (s, e) {
                    (State::Running(s), Event::Stopped(e)) => {
                        Self::on_running_stopped(s, e).map(State::Idle)
                    }
                    (State::Idle(s), Event::Started(e)) => {
                        Self::on_idle_started(s, e).map(State::Running)
                    }
                    _ => None,
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

            // Let's implement this optional function to show how entry/exit
            // processing can be achieved, and also confirm that our FSM is
            // calling it.
            async fn on_exit(old_s: &State, se: &mut EffectHandlers) {
                if let State::Running(s) = old_s {
                    Self::on_exit_running(s, se)
                }
            }

            // [mem::discriminant] is used to compare an old state's and a new state's
            // variants given that `State` represents an enum.
            // [mem::discriminant]'s use where `State` is not an enum is undefined, which would
            // therefore warrant a different implementation of this function.
            fn is_transitioning(s0: &State, s1: &State) -> bool {
                core::mem::discriminant(s0) != core::mem::discriminant(s1)
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

            fn on_exit_running(_old_s: &Running, se: &mut EffectHandlers) {
                se.exit_running()
            }

            fn on_running_stopped(_s: &Running, _e: &Stopped) -> Option<Idle> {
                Some(Idle)
            }

            async fn for_idle_start(
                _s: &Idle,
                _c: Start,
                se: &mut EffectHandlers,
            ) -> Option<Started> {
                se.start_something().await;
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
            transitioned_started_to_stopped: 0,
        };

        // Finally, test the FSM by stepping through various states

        let (e, t) = MyFsm::step(&State::Idle(Idle), Command::Start(Start), &mut se).await;
        assert!(matches!(e, Some(Event::Started(Started))));
        assert!(matches!(t, Some(State::Running(Running))));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_started_to_stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Running(Running), Command::Start(Start), &mut se).await;
        assert!(e.is_none());
        assert!(t.is_none());
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_started_to_stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Running(Running), Command::Stop(Stop), &mut se).await;
        assert!(matches!(e, Some(Event::Stopped(Stopped))));
        assert!(matches!(t, Some(State::Idle(Idle))));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_started_to_stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Idle(Idle), Command::Stop(Stop), &mut se).await;
        assert!(e.is_none());
        assert!(t.is_none());
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_started_to_stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);
    }
}
