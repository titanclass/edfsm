# Store events in sqlite

This is an alternative to [streambed]() to provide persistance to edfsm state machines.  An SQLite database is interfaced directly with `Machine` via its adapter traits.   The aim is to leverage many advantages of SQLite. The tradeoff is more limited log compaction options. The simplicity of the idea is clear.  This proof of concept stands at around 200 loc.  The log is just a single file database.  

## How it Works

Events are stored in two tables. New events are inserted into `log` which has `offset` as an _integer primary key_.  SQLite manages this special column and thereby keeps track of the current event offset automatically.  It can efficiently present events in offset order without needed an index.

A second table, `compacted` stores events by their compaction key.   Compaction is performed by an SQL statement that copies events in offset order from the `log` to `compacted`. Key conflicts are handled by accepting the most recent log record, which is the event with greatest offset.  After compaction the log is trimmed with an SQL delete statement.

The compaction policy resulting from this is to retain one event with each compaction key.  Events are replayed by querying the `log` table followed by events in `compacted` that are older than the last event in `log`.   

## Types and Traits

 A `BackingStore` wraps an SQLite connection and other bookeeping information. It implements the functionality describe so far. An `AsyncBackingStore` wraps a `BackingStore` in a mutex.  This type can be passed to a `Machine` to act as a persistent log.  It implements the necessary traits, `Adapter` and `Feed`.

 ## Rationale and Alternatives

 The idea of implementing this in streambed was explored and that remains a possibility.  The approach would be to implement `CommitLog`.  Instead this store implements `Adapter` and `Feed` which are more minimalist traits.   An important function of streambed is as an interprocess communication channel similar to _Kafka_.  This is not supported here at all and is not an SQLite strength.   Many streambed concerns are avoided including topics, groups and the metadata defined in `ProducerRecord`.

 The approach to async operation implemented here is about as primitive as can be.  `AsyncBackingStore` simply wraps the database connection in a mutex.   There is no buffering or streaming of events.   For the most part this seems acceptable.  The `Machine` will generall be surrounded by queues. One weakness is that, without streaming, event replay materializes the entire event history in memory.  
 

