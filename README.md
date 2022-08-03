edfsm - Event Driven Finite State Machine
===

Event driven Finite State Machines process commands (possibly created by other
events), possibly performing some side effect, and possibly emitting events.

Commands are processed against a provided state. Events can be applied to states
to yield new states.

For more background: [Event-driven Finite State Machines](http://christopherhunt-software.blogspot.com/2021/02/event-driven-finite-state-machines.html).

DSL
---

An attribute macro has been provided that provides a Domain Specific Language (DSL) mapping directly
from a Finite State Machine description to code. The goal of the DSL is to convey the Finite
State Machine in a way that can closely match its design. Given the macro, compilation also ensures that the correct
state, command and event types are handled by the developer.

Here is an example given the declaration of states, commands, events and an effect handler:

```rust
struct MyFsm {}

#[impl_fsm]
impl Fsm<State, Command, Event, EffectHandlers> for MyFsm {
    state!(Running / entry);
    state!(Running / exit);

    transition!(Idle    => Start => Started => Running);
    transition!(Running => Stop  => Stopped => Idle);
}
```

The `state!` macro declares state-related attributes. At this time, entry and exit
handlers can be declared. In our example, the macro will ensure that a `to_running`
and a `from_running` method will be called for `MyFsm`. The developer is then
required to implement these methods e.g.:

```rust
fn from_running(_old_s: &Running, _se: &mut EffectHandlers) {
    // Do something
}
```

The `transition!` macro declares an entire transition using the form:

```
<from-state> => <given-command> => <yields-event> => <to-state>
```

In our example, for the first transition, multiple methods will be called that the developer must provide e.g.:

```rust
fn for_idle_start_started(_s: &Idle, _c: Start, _se: &mut EffectHandlers) -> Option<Started> {
    // Perform some effect here if required. Effects are performed via the EffectHandler
    Some(Started)
}

fn for_idle_started_running(_s: &Idle, _e: &Started) -> Option<Running> {
    Some(Running)
}
```

Please see the event_driven/tests folder for complete examples.

no_std
---

The library is able to support`no_std`, particularly for usage on embedded targets.

## Contribution policy

Contributions via GitHub pull requests are gladly accepted from their original author. Along with any pull requests, please state that the contribution is your original work and that you license the work to the project under the project's open source license. Whether or not you state this explicitly, by submitting any copyrighted material via pull request, email, or other means you agree to license the material under the project's open source license and warrant that you have the legal authority to do so.

## License

This code is open source software licensed under the [Apache-2.0 license](./LICENSE).

© Copyright [Titan Class P/L](https://www.titanclass.com.au/), 2022
