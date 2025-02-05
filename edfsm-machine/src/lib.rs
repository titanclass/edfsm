#![doc = include_str!("../README.md")]
#![no_std]
#[cfg(feature = "std")]
extern crate std;

pub mod adapter;
pub mod error;

#[cfg(feature = "std")]
pub mod output;

#[cfg(feature = "tokio")]
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::{
    adapter::{Adapter, Feed, Placeholder},
    error::Result,
};
use core::future::Future;
use edfsm::{Drain, Fsm, Init, Input, Terminating};

/// The event type of an Fsm
pub type Event<M> = <M as Fsm>::E;

/// The command type of an Fsm
pub type Command<M> = <M as Fsm>::C;

/// The input type of an Fsm
pub type In<M> = Input<<M as Fsm>::C, <M as Fsm>::E>;

/// The output message type of an Fsm for the purpose of this module.
pub type Out<M> = <<M as Fsm>::SE as Drain>::Item;

/// The effector/effects type of an Fsm
pub type Effects<M> = <M as Fsm>::SE;

/// The state type of an Fsm
pub type State<M> = <M as Fsm>::S;

/// A `Machine` is a state machine (implementing `Fsm`) that will run in a rust `task`.
///
/// Each `Machine` has an input channel, and adapters for output and event log.
/// The type of the input messages, events and output messages are part of
/// the state machine specification, ie the `Fsm` implementation.
/// Conversely, the wiring or inputs and outputs is independent of the underlying state machine
/// and involves channels and adapters.
///
/// A `Machine` also has a data structure used to perform side effects, including generating output messages.
/// The type of this is also part of the state machine specification (the `SE` associated type).  
/// Note: side effects must be synchronous. If they may block they should be bracketed with
/// tokio's `block_in_place` or equivalent.
///
/// A machine is created by functions `machine` or `machine_with_effects`.
/// It is wired to other machines or channels by functions `input`, `with_output`, `merge_output` and
/// `with_event_log`.
///
/// The machine is made runnable by function `task`.  This is a future intended to be spawned onto
/// the tokio (or other) runtime.
///
/// Once running, a `Machine`
/// - initialises state, which may involve replaying messages from the event log
/// - performs initial effects
/// - enters the main loop, which is dirven by messages received on the input channel
/// - each message may cause the state to evolve and/or generate side effects
/// - an event is logged if the state changed
/// - any output messages are dispatched
///
pub trait Machine<M>
where
    M: Fsm,
    Effects<M>: Drain,
{
    /// Return a new `Sender` for the input channel.
    /// Any number can be created , enabling fan-in of messages.
    ///
    /// The sender accepts the Fsm `Input` values, representing either
    /// a command or an event.   It implements `Adapter` so the type can be adjusted.
    /// For example, to accept events only use:
    ///
    /// `machine.input().adapt_map(Input::Event)`
    ///
    fn input(&self) -> Sender<In<M>>;

    /// Connect a channel `Sender` or an adapter for output messages.
    ///
    /// This method replaces any existing adapter for output messages.
    /// Note that if the channel or adapter stalls this will stall the state machine.
    fn with_output(self, output: impl Adapter<Item = Out<M>> + 'static) -> impl Machine<M>;

    /// Connect an additional channel or adapter for output messages.
    ///
    /// Any number of channels or adapters can be connected, enabling fan-out of messages.
    /// Each will receive all output messages, however if an adapter stalls this will stall the state machine.
    fn merge_output(self, output: impl Adapter<Item = Out<M>> + 'static) -> impl Machine<M>
    where
        Out<M>: Clone + Send;

    /// Connect an event log that provides intialisation from historical events and records live events.
    ///
    /// Each event received by the machine and each event produced by a command will be notified.
    /// This method replaces any existing event log.
    fn with_event_log(
        self,
        log: impl Adapter<Item = Event<M>> + Feed<Item = Event<M>> + 'static,
    ) -> impl Machine<M>;

    /// Connect an additional channel or adapter for events.
    ///
    /// Each event received by the machine and each event produced by a command will be notified.
    /// Any number of channels or adapters can be connected, enabling fan-out of events.
    /// Each will receive all output messages, however if an adapter stalls this will stall the state machine.
    fn merge_event_log(self, output: impl Adapter<Item = Event<M>> + 'static) -> impl Machine<M>;

    /// Convert this machine into a future that will run as a task
    fn task(self) -> impl Future<Output = Result<()>> + Send + 'static
    where
        Self: Sized,
        Out<M>: Send,
        Event<M>: Send + Terminating,
        Effects<M>: Init<State<M>> + Send,
        Command<M>: Send,
        State<M>: Default + Send;
}

/// A concrete `Machine`
struct Template<M, N, O, P>
where
    M: Fsm,
{
    sender: Option<Sender<In<M>>>,
    receiver: Receiver<In<M>>,
    effects: Effects<M>,
    log: N,
    output: O,
    events: P,
}

impl<M, N, O, P> Machine<M> for Template<M, N, O, P>
where
    M: Fsm + 'static,
    Effects<M>: Drain,
    N: Adapter<Item = Event<M>> + Feed<Item = Event<M>> + 'static,
    O: Adapter<Item = Out<M>> + 'static,
    P: Adapter<Item = Event<M>> + 'static,
    Event<M>: Clone + Send,
{
    fn input(&self) -> Sender<In<M>> {
        self.sender.as_ref().unwrap().clone()
    }

    fn with_output(self, output: impl Adapter<Item = Out<M>> + 'static) -> impl Machine<M> {
        Template {
            sender: self.sender,
            receiver: self.receiver,
            effects: self.effects,
            log: self.log,
            output,
            events: self.events,
        }
    }

    fn merge_output(self, output: impl Adapter<Item = Out<M>> + 'static) -> impl Machine<M>
    where
        Out<M>: Clone + Send,
    {
        Template {
            sender: self.sender,
            receiver: self.receiver,
            effects: self.effects,
            log: self.log,
            output: self.output.merge(output),
            events: self.events,
        }
    }

    fn with_event_log(
        self,
        log: impl Adapter<Item = Event<M>> + Feed<Item = Event<M>> + 'static,
    ) -> impl Machine<M> {
        Template {
            sender: self.sender,
            receiver: self.receiver,
            effects: self.effects,
            log,
            output: self.output,
            events: self.events,
        }
    }

    fn merge_event_log(self, events: impl Adapter<Item = Event<M>> + 'static) -> impl Machine<M> {
        Template {
            sender: self.sender,
            receiver: self.receiver,
            effects: self.effects,
            log: self.log,
            output: self.output,
            events: self.events.merge(events),
        }
    }

    async fn task(mut self) -> Result<()>
    where
        Effects<M>: Init<State<M>>,
        State<M>: Default,
        Event<M>: Send + Terminating,
        State<M>: Send,
    {
        // close the local sender side of the input channel
        // this ensures the task will exit when all other senders are closed
        self.sender = None;

        // Construct the initial state and rehydrate it from the log.
        let mut state: State<M> = Default::default();
        let mut hydra = Hydrator::<M> { state: &mut state };
        self.log.feed(&mut hydra).await?;

        // Initialise the effector with the rehydrated, state.
        self.effects.init(&state);

        // Flush output messages generated in initialisation
        for item in self.effects.drain_all() {
            self.output.notify(item).await
        }

        // Read events and commands
        while let Some(input) = self.receiver.recv().await {
            // Indicates a terminating event is seen
            let mut terminating = false;

            // Run Fsm and log any event
            if let Some(e) = M::step(&mut state, input, &mut self.effects) {
                terminating = e.terminating();
                self.log.clone_notify(&e).await;
                self.events.notify(e).await;
            }

            // Flush output messages generated during the `step`, if any.
            for item in self.effects.drain_all() {
                self.output.notify(item).await
            }

            if terminating {
                break;
            }
        }
        Ok(())
    }
}

/// Default machine input backlog limit
pub const DEFAULT_BUFFER: usize = 10;

/// Create new machine for an `Fsm` of type `M`
pub fn machine<M>() -> impl Machine<M>
where
    M: Fsm + 'static,
    Effects<M>: Drain + Default,
    Out<M>: Send + Clone,
    Event<M>: Send + Sync + Clone,
{
    machine_with_effects(Default::default(), DEFAULT_BUFFER)
}

/// Create a new machine for an `Fsm` of type `M` with explicit effects and backlog
pub fn machine_with_effects<M>(effects: Effects<M>, buffer: usize) -> impl Machine<M>
where
    M: Fsm + 'static,
    Effects<M>: Drain,
    Out<M>: Send + Clone,
    Event<M>: Send + Sync + Clone,
{
    let (sender, receiver) = channel(buffer);
    Template {
        sender: Some(sender),
        receiver,
        effects,
        log: Placeholder::default(),
        output: Placeholder::default(),
        events: Placeholder::default(),
    }
}

/// A `Hydrator` is an event `Adapter` that accepts
/// a stream of initialisation events for an `Fsm`.
///
/// It will apply these to the state bringing it up
/// to date without causing side effects.
struct Hydrator<'a, M>
where
    M: Fsm,
{
    state: &'a mut State<M>,
}

impl<M> Adapter for Hydrator<'_, M>
where
    M: Fsm,
    Event<M>: Send,
    State<M>: Send,
{
    type Item = Event<M>;

    async fn notify(&mut self, a: Self::Item)
    where
        Self::Item: Send + 'static,
    {
        M::on_event(self.state, &a);
    }
}
