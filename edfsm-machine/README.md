# Event driven FSM Machine

`edfsm-machine` effectively implements the [Actor Model](https://en.wikipedia.org/wiki/Actor_model) for Rust, 
where its messages are the inputs, events and outputs.

`edfsm-machine` provides a convenient way to drive an `edfsm`-based finite state machine with inputs (commands and events),
and a means to capture its resulting events and "outputs". Outputs are the consequence of invoking `edfsm`'s side-effect
handling, and usually means capturing an output buffer of effects.

Upon initialising the machine and wiring up inputs, event logs and outputs, a future is produced that can be spawned
by executors such as those provided by [tokio](https://github.com/tokio-rs/tokio).

Taking this further, a machine's inputs can be conveniently sourced from a [streambed-logged](https://github.com/streambed/streambed-rs/tree/main/streambed-logged)
log of events that have been persisted, and logged back there. These adaptations provides an [event-sourcing](https://martinfowler.com/eaaDev/EventSourcing.html)-based Actor Model.

This library assumes no_std and requires features such as `tokio` to make it useful.