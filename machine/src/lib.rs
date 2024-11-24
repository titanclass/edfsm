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
use edfsm::{Drain, Fsm, Init, Input};

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

/// A `Machine` is a state machine (implementing `Fsm`) that runs in a rust `task`.
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
pub trait Machine
where
    Self::M: Fsm,
    Effects<Self::M>: Drain,
{
    type M;

    fn input(&self) -> Sender<In<Self::M>>;
    fn with_output(
        self,
        output: impl Adapter<Item = Out<Self::M>> + 'static,
    ) -> impl Machine<M = Self::M>;
    fn with_event_log(
        self,
        log: impl Adapter<Item = Event<Self::M>> + Feed<Item = Event<Self::M>> + 'static,
    ) -> impl Machine<M = Self::M>;
    fn merge_output(
        self,
        output: impl Adapter<Item = Out<Self::M>> + 'static,
    ) -> impl Machine<M = Self::M>
    where
        Out<Self::M>: Clone + Send;
    fn task(self) -> impl Future<Output = Result<()>> + Send + 'static
    where
        Self: Sized,
        Out<Self::M>: Send,
        Event<Self::M>: Send,
        Effects<Self::M>: Init<State<Self::M>> + Send,
        Command<Self::M>: Send,
        State<Self::M>: Default + Send;
}

pub struct Template<M, N, O>
where
    M: Fsm,
{
    sender: Option<Sender<In<M>>>,
    receiver: Receiver<In<M>>,
    effects: Effects<M>,
    logger: N,
    output: O,
}

impl<M, N, O> Machine for Template<M, N, O>
where
    M: Fsm + 'static,
    Effects<M>: Drain,
    N: Adapter<Item = Event<M>> + Feed<Item = Event<M>> + 'static,
    O: Adapter<Item = Out<M>> + 'static,
{
    type M = M;

    fn input(&self) -> Sender<In<Self::M>> {
        self.sender.as_ref().unwrap().clone()
    }

    fn with_output(
        self,
        output: impl Adapter<Item = Out<Self::M>> + 'static,
    ) -> impl Machine<M = Self::M> {
        Template {
            sender: self.sender,
            receiver: self.receiver,
            effects: self.effects,
            logger: self.logger,
            output,
        }
    }

    fn merge_output(
        self,
        output: impl Adapter<Item = Out<Self::M>> + 'static,
    ) -> impl Machine<M = Self::M>
    where
        Out<Self::M>: Clone + Send,
    {
        Template {
            sender: self.sender,
            receiver: self.receiver,
            effects: self.effects,
            logger: self.logger,
            output: self.output.merge(output),
        }
    }

    fn with_event_log(
        self,
        log: impl Adapter<Item = Event<Self::M>> + Feed<Item = Event<Self::M>> + 'static,
    ) -> impl Machine<M = Self::M> {
        Template {
            sender: self.sender,
            receiver: self.receiver,
            effects: self.effects,
            logger: log,
            output: self.output,
        }
    }

    async fn task(mut self) -> Result<()>
    where
        Effects<Self::M>: Init<State<Self::M>>,
        State<Self::M>: Default,
        Event<M>: Send,
        State<M>: Send,
    {
        // close the local sender side of the input channel
        // this ensures the task will exit when all other senders are closed
        self.sender = None;

        // Construct the initial state and rehydrate it from the log.
        let mut state: State<M> = Default::default();
        let mut hydra = Hydrator::<M> { state: &mut state };
        self.logger.feed(&mut hydra).await?;
        drop(hydra);

        // Initialise the effector with the rehydrated, state.
        self.effects.init(&state);

        // Flush output messages generated in initialisation
        for item in self.effects.drain_all() {
            self.output.notify(item).await?
        }

        // Read events and commands
        while let Some(input) = self.receiver.recv().await {
            // Run Fsm and log any event
            if let Some(e) = M::step(&mut state, input, &mut self.effects) {
                self.logger.notify(e).await?;
            }

            // Flush output messages generated during the `step`, if any.
            for item in self.effects.drain_all() {
                self.output.notify(item).await?
            }
        }
        Ok(())
    }
}

/// Default machine input backlog limit
pub const DEFAULT_BUFFER: usize = 10;

pub fn machine<M>() -> impl Machine<M = M>
where
    M: Fsm + 'static,
    Effects<M>: Drain + Default,
    Out<M>: Send + Clone,
    Event<M>: Send + Sync,
{
    machine_with_effects(Default::default(), DEFAULT_BUFFER)
}

pub fn machine_with_effects<M>(effects: Effects<M>, buffer: usize) -> impl Machine<M = M>
where
    M: Fsm + 'static,
    Effects<M>: Drain,
    Out<M>: Send + Clone,
    Event<M>: Send + Sync,
{
    let (sender, receiver) = channel(buffer);
    Template {
        sender: Some(sender),
        receiver,
        effects,
        logger: Placeholder::default(),
        output: Placeholder::default(),
    }
}

pub struct Builder<M, N = Placeholder<Event<M>>, O = Placeholder<Out<M>>>
where
    M: Fsm,
    Effects<M>: Drain,
{
    state: State<M>,
    sender: Option<Sender<In<M>>>,
    receiver: Receiver<In<M>>,
    effector: Effects<M>,
    logger: N,
    output: O,
}

impl<M> Default for Builder<M>
where
    M: Fsm,
    Effects<M>: Drain + Default,
    State<M>: Default,
{
    fn default() -> Self {
        Self::new(Default::default(), DEFAULT_BUFFER)
    }
}

impl<M> Builder<M>
where
    M: Fsm,
    Effects<M>: Drain,
    State<M>: Default,
{
    /// Construct a machine from an explicit, possibly non-default, effector,
    /// and an explicit input buffer size.
    pub fn new(effector: Effects<M>, buffer: usize) -> Self {
        let (sender, receiver) = channel(buffer);
        Builder {
            state: Default::default(),
            sender: Some(sender),
            receiver,
            effector,
            logger: Default::default(),
            output: Default::default(),
        }
    }
}

impl<M, N, O> Builder<M, N, O>
where
    M: Fsm,
    Effects<M>: Drain,
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
    pub fn connect_event_log<T>(self, logger: T) -> Builder<M, impl Adapter<Item = Event<M>>, O>
    where
        T: Adapter<Item = Event<M>>,
        N: Adapter<Item = Event<M>>,
        Event<M>: Send + Clone,
    {
        Builder {
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
    pub fn connect_output<T>(self, output: T) -> Builder<M, N, impl Adapter<Item = Out<M>>>
    where
        T: Adapter<Item = Out<M>>,
        O: Adapter<Item = Out<M>>,
        Out<M>: Send + Clone,
    {
        Builder {
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
    pub fn task(mut self) -> impl Future<Output = Result<()>>
    where
        N: Adapter<Item = Event<M>>,
        O: Adapter<Item = Out<M>>,
        Event<M>: Clone + Send + 'static,
        Out<M>: Clone + Send + 'static,
        Effects<M>: Init<State<M>> + Send,
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
            for item in self.effector.drain_all() {
                self.output.notify(item).await?
            }

            // Read events and commands
            while let Some(input) = self.receiver.recv().await {
                // Run Fsm and log any event
                if let Some(e) = M::step(&mut self.state, input, &mut self.effector) {
                    self.logger.notify(e).await?;
                }

                // Flush output messages generated during the `step`, if any.
                for item in self.effector.drain_all() {
                    self.output.notify(item).await?
                }
            }
            Ok(())
        }
    }
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
        Self::Item: Send + 'static,
    {
        M::on_event(self.state, &a);
        Ok(())
    }
}

#[cfg(feature = "streambed")]
mod commit_log {
    use crate::{Adapter, Builder, Drain, Effects, Event};
    use edfsm::Fsm;
    use futures_util::StreamExt;
    use streambed_machine::{Codec, CommitLog, LogAdapter};

    impl<M, N, O> Builder<M, N, O>
    where
        M: Fsm,
        Effects<M>: Drain,
        N: Adapter<Item = Event<M>>,
        Event<M>: Send + Sync + Clone + 'static,
    {
        /// Connect this Fsm to a streambed `CommitLog` and initialise its state.
        pub async fn initialise<L, C>(
            mut self,
            log: LogAdapter<L, C, Event<M>>,
        ) -> Builder<M, impl Adapter<Item = Event<M>>, O>
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
