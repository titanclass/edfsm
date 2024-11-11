#![no_std]
#[cfg(feature = "std")]
extern crate std;

pub mod adapter;
pub mod error;

use crate::{
    adapter::{Adapter, Discard},
    error::Result,
};
use core::future::Future;
use edfsm::{Fsm, Input};

#[cfg(feature = "tokio")]
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// The event type of an Fsm
pub type Event<M> = <M as Fsm>::E;

/// The command type of an Fsm
pub type Command<M> = <M as Fsm>::C;

/// The input type of an Fsm
pub type In<M> = Input<<M as Fsm>::C, <M as Fsm>::E>;

/// The output message type of an Fsm for the purpose of this module.
pub type Out<M> = <<M as Fsm>::SE as Drain>::Item;

/// The effector/effects type of an Fsm
pub type Effect<M> = <M as Fsm>::SE;

/// The state type of an Fsm
pub type State<M> = <M as Fsm>::S;

/// A `Machine` is a state machine (implementing `Fsm`) running in a rust `task`.
///
/// Each `Machine` has a current state, an input channel, an effector and adapters.
/// The type of the state and the channel are part of the state machine specification.
/// As messages arrive on the channel, the state machine `Fsm::step` method
/// evolves the state and orchestrates side effects.
///
/// The effector enables the machine to execute side effects and its type is also part of
/// the state machine specification, `Fsm::SE`.  Generally, side effects must
/// be synchronous and if they may block they should be bracketed with tokio's `block_in_place`.
///
/// The adapters are for communication and event logging.
/// A machine's inputs and outputs can be wired up without affecting the underlying state machine.  
/// Communication is always asynchronous and the adapter `notify` method is `async`.  
pub struct Machine<M, N = Discard<Event<M>>, O = Discard<Out<M>>>
where
    M: Fsm,
    Effect<M>: Drain,
{
    state: State<M>,
    sender: Option<Sender<In<M>>>,
    receiver: Receiver<In<M>>,
    effector: Effect<M>,
    logger: N,
    output: O,
}

/// Default machine input backlog limit
pub const DEFAULT_BUFFER: usize = 10;

impl<M> Default for Machine<M>
where
    M: Fsm,
    Effect<M>: Drain + Default,
    State<M>: Default,
{
    fn default() -> Self {
        Self::new(Default::default(), DEFAULT_BUFFER)
    }
}

impl<M> Machine<M>
where
    M: Fsm,
    Effect<M>: Drain,
    State<M>: Default,
{
    /// Construct a machine from an explicit, possibly non-default, effector,
    /// and an explicit input buffer size.
    pub fn new(effector: Effect<M>, buffer: usize) -> Self {
        let (sender, receiver) = channel(buffer);
        Machine {
            state: Default::default(),
            sender: Some(sender),
            receiver,
            effector,
            logger: Default::default(),
            output: Default::default(),
        }
    }
}

impl<M, N, O> Machine<M, N, O>
where
    M: Fsm,
    Effect<M>: Drain,
{
    /// Return an adapter for initialisation events.  
    ///
    /// Events received will be applied to the state without causing side effects,
    /// reconstructing or rehydrating the state. Then the adapter should be dropped.
    ///
    /// This is an optional step before the machine is converted to a task.
    /// Remain effector-specific initialisation occurs in the task.
    pub fn initial_events(&mut self) -> Hydrator<M> {
        Hydrator {
            state: &mut self.state,
        }
    }

    /// Return a new `Sender` for the input channel.
    /// Any number can be created , enabling fan-in of messages.
    ///
    /// The sender accepts the Fsm `Input` values, representing either
    /// a command or an event.   It implements `Adapter` so the type can be adjusted.
    /// For example, to accept events only use:
    ///
    /// `machine.input().adapt_map(Input::Event)`
    ///
    pub fn input(&self) -> Sender<In<M>> {
        self.sender.as_ref().unwrap().clone()
    }

    /// Connect a channel sender or adapter for logging events.  
    ///
    /// Any number of channels or adapters can be connected, enabling fan-out of messages.
    /// Each will receive all events, however if an adapter stalls this will stall the state machine.
    pub fn connect_event_log<T>(self, logger: T) -> Machine<M, impl Adapter<Item = Event<M>>, O>
    where
        T: Adapter<Item = Event<M>>,
        N: Adapter<Item = Event<M>>,
        Event<M>: Send,
    {
        Machine {
            state: self.state,
            sender: self.sender,
            receiver: self.receiver,
            effector: self.effector,
            logger: self.logger.merge(logger),
            output: self.output,
        }
    }

    /// Connect a channel sender or adapter for output messages.
    ///
    /// Any number of channels or adapters can be connected, enabling fan-out of messages.
    /// Each will receive all output messages, however if an adapter stalls this will stall the state machine.
    pub fn connect_output<T>(self, output: T) -> Machine<M, N, impl Adapter<Item = Out<M>>>
    where
        T: Adapter<Item = Out<M>>,
        O: Adapter<Item = Out<M>>,
        Out<M>: Send,
    {
        Machine {
            state: self.state,
            sender: self.sender,
            receiver: self.receiver,
            effector: self.effector,
            logger: self.logger,
            output: self.output.merge(output),
        }
    }

    /// Convert this machine into a future that will run as a task
    #[allow(clippy::manual_async_fn)]
    pub fn task(mut self) -> impl Future<Output = Result<()>> + Send
    where
        N: Adapter<Item = Event<M>>,
        O: Adapter<Item = Out<M>>,
        Event<M>: Clone + Send + 'static,
        Out<M>: Clone + Send + 'static,
        Effect<M>: Init<State<M>> + Send,
        State<M>: Send,
        Command<M>: Send,
    {
        async move {
            // close the local sender side of the input channel
            // this ensures the task will exit when all other senders are closed
            self.sender = None;

            // Initialise the effector with the, possibly rehydrated, state.
            self.effector.init(&self.state);

            // Flush output messages generated in initialisation
            for item in self.effector.drain_all()? {
                self.output.notify(item).await?
            }

            // Read events and commands
            while let Some(input) = self.receiver.recv().await {
                // Run Fsm and log any event
                if let Some(e) = M::step(&mut self.state, input, &mut self.effector) {
                    self.logger.notify(e).await?;
                }

                // Flush output messages generated during the `step`, if any.
                for item in self.effector.drain_all()? {
                    self.output.notify(item).await?
                }
            }
            Ok(())
        }
    }
}

/// The ability to extract messages.
///
/// This trait is required for `Fsm::SE` to collect output from the effector.
pub trait Drain {
    /// Messages generated in the effector
    type Item;

    /// remove and return accumulated messages.
    fn drain_all(&mut self) -> Result<impl Iterator<Item = Self::Item> + Send>
    where
        Self::Item: Send;
}
/// The ability to initialize with a given, starting _state_ value.
///
/// This trait is required for `Fsm::SE` by the `hydrate` method.
pub trait Init<S> {
    fn init(&mut self, state: &S);
}

/// A `Hydrator` is an event `Adapter` that accepts
/// a stream of initialisation events for an `Fsm`.
///
/// It will apply these to the state bringing it up
/// to date without causing side effects.
pub struct Hydrator<'a, M>
where
    M: Fsm,
{
    state: &'a mut State<M>,
}

impl<'a, M> Adapter for Hydrator<'a, M>
where
    M: Fsm,
    Event<M>: Send,
    State<M>: Send,
{
    type Item = Event<M>;

    async fn notify(&mut self, a: Self::Item) -> Result<()>
    where
        Self::Item: Clone + Send + 'static,
    {
        M::on_event(self.state, &a);
        Ok(())
    }
}

/// Implement effector traits for a std Vec.
#[cfg(feature = "std")]
pub mod output_vec {
    use crate::{Drain, Init, Result};

    impl<A> Drain for std::vec::Vec<A> {
        type Item = A;

        fn drain_all(&mut self) -> Result<impl Iterator<Item = Self::Item> + Send>
        where
            Self::Item: Send,
        {
            Ok(self.drain(0..))
        }
    }

    impl<S, A> Init<S> for std::vec::Vec<A> {
        fn init(&mut self, _: &S) {}
    }
}

#[cfg(feature = "streambed")]
mod commit_log {
    use crate::{Adapter, Drain, Effect, Event, Machine};
    use edfsm::Fsm;
    use futures_util::StreamExt;
    use streambed_machine::{Codec, CommitLog, CompactionKey, LogAdapter};

    impl<M, N, O> Machine<M, N, O>
    where
        M: Fsm,
        Effect<M>: Drain,
        N: Adapter<Item = Event<M>>,
        Event<M>: Send + Sync + CompactionKey + Clone + 'static,
    {
        /// Connect this Fsm to a streambed `CommitLog` and initialise its state.
        pub async fn initialize<L, C>(
            mut self,
            log: LogAdapter<L, C, Event<M>>,
        ) -> Machine<M, impl Adapter<Item = Event<M>>, O>
        where
            L: CommitLog + Send + Sync,
            C: Codec<Event<M>> + Send + Sync,
        {
            let mut events = log.history().await;
            while let Some(e) = events.next().await {
                M::on_event(&mut self.state, &e);
            }
            drop(events);
            self.connect_event_log(log)
        }
    }
}
