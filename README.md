\* \* \* EXPERIMENTAL \* \* \*

This project is to be considered experimental and incomplete.

edfsm - Event Driven Finite State Machine
===

Event driven Finite State Machines process commands (possibly created by other
events), performing some side effect, and emitting events.
Commands are processed against a provided state. Events can be applied to states
to yield new states.

For more background: [Event-driven Finite State Machines](http://christopherhunt-software.blogspot.com/2021/02/event-driven-finite-state-machines.html).

DSL
---

A macro has been provided that provides a Domain Specific Language mapping directly
from a Finite State Machine description to code. Please see the event_driven/tests folder for
examples.

no_std
---

The library is able to support`no_std`, particularly for usage on embedded targets.

## Contribution policy

Contributions via GitHub pull requests are gladly accepted from their original author. Along with any pull requests, please state that the contribution is your original work and that you license the work to the project under the project's open source license. Whether or not you state this explicitly, by submitting any copyrighted material via pull request, email, or other means you agree to license the material under the project's open source license and warrant that you have the legal authority to do so.

## License

This code is open source software licensed under the [Apache-2.0 license](./LICENSE).

© Copyright [Titan Class P/L](https://www.titanclass.com.au/), 2022
