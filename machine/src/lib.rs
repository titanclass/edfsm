#![no_std]
#[cfg(feature = "std")]
extern crate std;

pub mod adapter;
pub mod error;

use crate::{
    adapter::{Adapter, Discard},
    error::Result,
};
use adapter::AdaptChannel;
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
/// The adapters are for outbound communication including event logging.
/// A machine's inputs and outputs can be wired up without changing the state machine.  
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
    /// Construct a machine from an explicit, possibly non-default, effector.
    /// This is also the constructor for a non-default input backlog value.
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

    /// Return an adapter for input messages, which may be commands or events.
    /// Any number of input adapters can be created, enabling fan-in of messages.
    /// Messages received on any input adapter are enqued to the underlying state machine.
    pub fn input(&self) -> impl Adapter<Item = In<M>>
    where
        In<M>: Clone + 'static,
    {
        AdaptChannel::new(self.sender.as_ref().unwrap().clone())
    }

    /// Return an adapter for input events only. (See `input()`)
    pub fn input_events(&self) -> impl Adapter<Item = Event<M>>
    where
        In<M>: Clone + 'static,
    {
        self.input().adapt_map(Input::Event)
    }

    /// Return an adapter for input commands only. (See `input()`)
    pub fn input_commands(&self) -> impl Adapter<Item = Command<M>>
    where
        In<M>: Clone + 'static,
    {
        self.input().adapt_map(Input::Command)
    }

    /// Connect an adapter for logging events.  
    /// Any number of adapters can be connected, enabling fan-out of messages.
    /// Each adapter will receive all events,
    /// however if an adapter stalls this will stall the state machine.
    pub fn connect_event_log<T>(self, logger: T) -> Machine<M, impl Adapter<Item = Event<M>>, O>
    where
        T: Adapter<Item = Event<M>>,
        N: Adapter<Item = Event<M>>,
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

    /// Connect adapter for output messages
    /// Any number of adapters can be connected, enabling fan-out of messages.
    /// Each adapter will receive all output messages,
    /// however if an adapter stalls this will stall the state machine.
    pub fn connect_output<T>(self, output: T) -> Machine<M, N, impl Adapter<Item = Out<M>>>
    where
        T: Adapter<Item = Out<M>>,
        O: Adapter<Item = Out<M>>,
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

    /// Access the sender side of the machine input channel
    pub fn sender(&self) -> &Sender<In<M>> {
        // Note: sender is always present when this method is accessable
        self.sender.as_ref().unwrap()
    }

    /// Convert this machine into a future that will run as a task
    pub async fn task(mut self) -> Result<()>
    where
        N: Adapter<Item = Event<M>>,
        O: Adapter<Item = Out<M>>,
        Event<M>: Clone + 'static,
        Out<M>: Clone + 'static,
        Effect<M>: Init<State<M>>,
    {
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

            // Flush output messages generated by a command, if any.
            for item in self.effector.drain_all()? {
                self.output.notify(item).await?
            }
        }
        Ok(())
    }
}

/// The ability to extract messages.
///
/// This trait is required for `Fsm::SE` to collect output from the effector.
pub trait Drain {
    /// Messages generated in the effector
    type Item;

    /// remove and return accumulated messages.
    fn drain_all(&mut self) -> Result<impl Iterator<Item = Self::Item>>;
}

#[cfg(feature = "std")]
impl<A> Drain for std::vec::Vec<A> {
    type Item = A;

    fn drain_all(&mut self) -> Result<impl Iterator<Item = Self::Item>> {
        Ok(self.drain(0..))
    }
}

/// The ability to initialize from a _state_ value.
///
/// This trait is required for `Fsm::SE` by the `hydrate` method.
pub trait Init<S> {
    fn init(&mut self, state: &S);
}

/// A `Hydrator` is an event `Adapter` that accepts
/// a stream of initialisation events for a `Fsm`.
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
{
    type Item = Event<M>;

    async fn notify(&mut self, a: Self::Item) -> Result<()>
    where
        Self::Item: Clone + 'static,
    {
        M::on_event(self.state, &a);
        Ok(())
    }
}
