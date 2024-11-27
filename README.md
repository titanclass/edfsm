edfsm - Event Driven Finite State Machine
===

An Event Driven Finite State Machine is a useful formalism for control and monitoring applications.  The key concepts are:

- The _state_ maintained by the state machine represents aspects of its external environment and its history.   
- An _event_ represents a change in the environment and is one form of input to the state machine.  Reception of an event may cause the state to be updated.
- A _command_ is an input to the state machine that may cause it to perform an effect.  An event may also be produced by the command which may update the state.
- An _effect_ is some action that influences the environment.  

The effect, if any, produced by a command will depend on both the command and the a-priori state.  This models the _proactive_ behaviour of a [Mealy](https://en.wikipedia.org/wiki/Mealy_machine) machine.  An effect may also be produced after and event is processed and it will depend on the a-posteriori state.  This models the _reactive_ behaviour of a [Moore](https://en.wikipedia.org/wiki/Moore_machine) machine.

Why edfsm?
---

edfsm, and its DSL in particular, help you identify the functions required to handle commands and events given
declared states, and strongly type their declarations. In short, edfsm is designed to enhance the code quality 
of your state machine by leveraging the compiler to assert your declaration of its transitions.

DSL
---

An attribute macro has been provided that provides a Domain Specific Language (DSL) mapping directly
from a Finite State Machine description to code. The goal of the DSL is to convey the Finite
State Machine in a way that can closely match its design. Given the macro, compilation also ensures that the correct
state, command and event types are handled by the developer.

Here is an example given the declaration of states, commands, events and an effect handler:

```rust,ignore
struct MyFsm;

#[impl_fsm]
impl Fsm for MyFsm {
    type S = State;
    type C = Command;
    type E = Event;
    type SE = EffectHandlers;

    state!(Running / entry);

    command!(Idle    => Start => Started => Running);
    command!(Running => Stop  => Stopped => Idle);

    ignore_command!(Idle    => Stop);
    ignore_command!(Running => Start);
}
```

The `state!` macro declares state-related attributes. At this time, entry 
handlers can be declared. In our example, the macro will ensure that a `on_entry_running`
method will be called for `MyFsm`. The developer is then
required to implement these methods e.g.:

```rust,ignore
fn on_entry_running(_old_s: &Running, _se: &mut EffectHandlers) {
    // Do something
}
```

The `command!` macro declares what should happen given a command using the form:

```compile_fail
<from-state> => <given-command> [=> <yields-event> [=> <to-state>]]
```

> When declaring states it is also possible to use a wildcard i.e. `_` in place of `<from-state>` and `<to-state>`.

In our example, for the first step declaration, multiple methods will be called that the developer must provide e.g.:

```rust,ignore
fn for_idle_start(_s: &Idle, _c: Start, _se: &mut EffectHandlers) -> Option<Started> {
    // Perform some effect here if required. Effects are performed via the EffectHandler
    Some(Started)
}

fn on_idle_started(_s: &Idle, _e: &Started) -> Option<Running> {
    Some(Running)
}
```

> Note that steps may also be declared for events using a `event!` macro (not shown). The form then becomes:
> 
> ```compile_fail
> <from-state> => <given-event> [=> <to-state> [ / action]]
> ```
>
> (`/ action` can be used to declare that a side-effect is to be performed)

The `ignore_command!` macro describes those states and commands that should be ignored given:

```compile_fail
<from-state> => <given-command>
```

> Note if no `ignore_command!` declarations are provided then exhaustive matching on states and commands is not enforced.

> There is a `ignore_event!` macro available for ignoring events where events are providing the input.

State machines are then advanced given a mutable state and command. An optional event can be
emitted along with a possible state transition e.g.:

```rust,ignore
let mut s = State::Idle(Idle);
let c = Command::Start(Start);
// Now step the state machine with the state and command,
// and, an (undeclared) effect handler.
let (e, t) = MyFsm::step(&mut s, Input::Command(c), &mut se);
```

State can also be re-constituted by replaying events. If there is no transition to an entirely
new state then the existing state may still have been updated.
Here is an example of applying an event to state with the update of state
if necessary and an option of `r` indicating some type of change that occurred, or `None` otherwise.

```rust,ignore
let r = MyFsm::on_event(&mut s, &e);
```

Mutating state can be very useful where a state itself represents
a finer granularity of state with its fields, and so we wish to update them directly. 
For example, given our previous representation of:

```rust,ignore
command!(Running => Stop  => Stopped => Idle);
```

...if we change it to:

```rust,ignore
command!(Running => Stop  => Stopped);
```

i.e. if we remove the target state, then the associated function will be able to mutate the
state and no transition can be returned as they are mutually exclusive actions. Here is
a sample signature in accordance with the above `command!`.

```rust,ignore
fn on_idle_started(s: &mut Idle, e: &Started) {
    // `s` can now be mutated given some `e`.
}
```

Please see the event_driven/tests folder for complete examples, including the ability to mutate
the passed state in the absence of a target state i.e. when emitting an event but not
transitioning.

edfsm-machine and edfsm-kv-store
---

These two crates provide a means to conveniently run an FSM of `edfsm` as a future, and also to
use an FSM as a key/value store respectively.

no_std
---

The library is able to support`no_std` and is designed for efficient usage with embedded targets.

## Contribution policy

Contributions via GitHub pull requests are gladly accepted from their original author. Along with any pull requests, please state that the contribution is your original work and that you license the work to the project under the project's open source license. Whether or not you state this explicitly, by submitting any copyrighted material via pull request, email, or other means you agree to license the material under the project's open source license and warrant that you have the legal authority to do so.

## License

This code is open source software licensed under the [Apache-2.0 license](./LICENSE).

© Copyright [Titan Class P/L](https://www.titanclass.com.au/), 2022-2024
