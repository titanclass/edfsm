edfsm - Event Driven Finite State Machine
===

Event driven Finite State Machines process commands and events (possibly created by other
events), possibly performing some side effect, and possibly emitting events.

In one scenario, commands are processed against a provided state. Events can be applied to states
to yield new states. This is known as a [Mealy](https://en.wikipedia.org/wiki/Mealy_machine) state machine. For more background: [Event-driven Finite State Machines](http://christopherhunt-software.blogspot.com/2021/02/event-driven-finite-state-machines.html).

In another scenario, events are applied to a provided state. This is known as a [Moore](https://en.wikipedia.org/wiki/Moore_machine)
state machine.

DSL
---

An attribute macro has been provided that provides a Domain Specific Language (DSL) mapping directly
from a Finite State Machine description to code. The goal of the DSL is to convey the Finite
State Machine in a way that can closely match its design. Given the macro, compilation also ensures that the correct
state, command and event types are handled by the developer.

Here is an example given the declaration of states, commands, events and an effect handler:

```rust
struct MyFsm;

#[impl_fsm]
impl Fsm for MyFsm {
    type S = State;
    type C = Command;
    type E = Event;
    type SE = EffectHandlers;

    state!(Running / entry);

    transition!(Idle    => Start => Started => Running);
    transition!(Running => Stop  => Stopped => Idle);

    ignore!(Idle    => Stop);
    ignore!(Running => Start);
}
```

The `state!` macro declares state-related attributes. At this time, entry 
handlers can be declared. In our example, the macro will ensure that a `on_entry_running`
method will be called for `MyFsm`. The developer is then
required to implement these methods e.g.:

```rust
fn on_entry_running(_old_s: &Running, _se: &mut EffectHandlers) {
    // Do something
}
```

The `transition!` macro declares an entire transition using the form:

```
<from-state> => <given-command> [=> <yields-event> []=> <to-state>]]
```

In our example, for the first transition, multiple methods will be called that the developer must provide e.g.:

```rust
fn for_idle_start(_s: &Idle, _c: Start, _se: &mut EffectHandlers) -> Option<Started> {
    // Perform some effect here if required. Effects are performed via the EffectHandler
    Some(Started)
}

fn on_idle_started(_s: &Idle, _e: &Started) -> Option<Running> {
    Some(Running)
}
```

The `ignore!` macro describes those states and commands that should be ignored given:

```
<from-state> => <given-command>
```

It is possible to use a wildcard i.e. `_` in place of `<from-state>` and `<to-state>`.

State machines are then advanced given a mutable state and command. An optional event can be
emitted along with a possible state transition e.g.:

```rust
let mut s = State::Idle(Idle);
let c = Command::Start(Start);
// Now step the state machine with the state and command,
// and, an (undeclared) effect handler.
let (e, t) = MyFsm::step(&mut s, Step::Command(c), &mut se);
```

State can also be re-constituted by replaying events. If there is no transition to an entirely
new state then the existing state may still have been updated.
Here is an example of applying an event to state with the update of state
if necessary and a bool of `t` indicating true if a transition occurred.

```rust
let t = MyFsm::on_event(&mut s, &e);
```

Mutating state can be very useful where a state itself represents
a finer granularity of state with its fields, and so we wish to update them directly. 
For example, given our previous representation of:

```rust
transition!(Running => Stop  => Stopped => Idle);
```

...if we change it to:

```rust
transition!(Running => Stop  => Stopped);
```

i.e. if we remove the target state, then the associated function will be able to mutate the
state and no transition can be returned as they are mutually exclusive actions. Here is
a sample signature in accordance with the above `transition`.

```rust
fn on_idle_started(s: &mut Idle, e: &Started) {
    // `s` can now be mutated given some `e`.
}
```

Please see the event_driven/tests folder for complete examples, including the ability to mutate
the passed state in the absence of a target state i.e. when emitting an event but not
transitioning.

no_std
---

The library is able to support`no_std` and is designed for efficient usage with embedded targets.

## Contribution policy

Contributions via GitHub pull requests are gladly accepted from their original author. Along with any pull requests, please state that the contribution is your original work and that you license the work to the project under the project's open source license. Whether or not you state this explicitly, by submitting any copyrighted material via pull request, email, or other means you agree to license the material under the project's open source license and warrant that you have the legal authority to do so.

## License

This code is open source software licensed under the [Apache-2.0 license](./LICENSE).

© Copyright [Titan Class P/L](https://www.titanclass.com.au/), 2022
