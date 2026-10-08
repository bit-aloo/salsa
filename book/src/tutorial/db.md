# Defining the database struct

First, we need to create the **database struct**.
Typically it is only used by the "driver" of your application;
the one which starts up the program, supplies the inputs, and relays the outputs.

In `calc`, the database struct is in the [`db`] module, and it looks like this:

[`db`]: https://github.com/salsa-rs/salsa/blob/master/examples/calc/db.rs

```rust
{{#include ../../../examples/calc/db.rs:db_struct}}
```

The `#[salsa::db]` attribute marks the struct as a database.
It must have a field named `storage` whose type is `salsa::Storage<Self>`, but it can also contain whatever other fields you want.

`salsa::Storage::new` optionally takes an event callback, which Salsa invokes as it works.
Our test configuration uses this to log a message each time a tracked function is about to execute
(`salsa::EventKind::WillExecute`).
Later in the tutorial, we use these logs to check which functions Salsa re-executes after an edit.
Outside of tests, `calc` uses `Default`, which creates the storage without a callback.

## Implementing the `salsa::Database` trait

In addition to the struct itself, we must add an impl of `salsa::Database`:

```rust
{{#include ../../../examples/calc/db.rs:db_impl}}
```
