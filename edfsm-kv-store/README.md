# Event Driven FSM KV Store

A [Key-Value Store](https://en.wikipedia.org/wiki/Key%E2%80%93value_database) based on edfsm's FSM trait implementation.
The store is often wired up to use `edfsm-machine` and `streambed-logged` to use as a fully-fledged persistent database. All key/values
are held in memory so only use for those scenarios where that is what is required.

This library uses `core` and `alloc`.