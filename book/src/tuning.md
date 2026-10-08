# Tuning Salsa

## Cache Eviction (LRU)

Salsa supports Least Recently Used (LRU) cache eviction for tracked functions.
By default, memoized values are never evicted (unbounded cache). You can enable
LRU eviction by specifying a capacity at compile time:

```rust
#[salsa::tracked(lru = 128)]
fn parse(db: &dyn Db, input: SourceFile) -> Ast {
    // ...
}
```

With `lru = 128`, Salsa will keep at most 128 memoized values for this function.
When the cache exceeds this capacity, the least recently used values are evicted
at the start of each new revision.

### Zero-Cost When Disabled

When no `lru` capacity is specified (the default), Salsa uses a no-op eviction
policy that is completely optimized away by the compiler. This means there is
zero runtime overhead for functions that don't need cache eviction.

### Runtime Capacity Adjustment

For functions with LRU enabled, you can adjust the capacity at runtime:

```rust
#[salsa::tracked(lru = 128)]
fn my_query(db: &dyn Db, input: MyInput) -> Output {
    // ...
}

// Later, adjust the capacity:
my_query::set_lru_capacity(&mut db, 256);
```

**Note:** The `set_lru_capacity` method is only generated for functions that have
an `lru` attribute. Functions without LRU enabled do not have this method.
The method is private to the module that defines the tracked function, so call it from
that module (or expose it through a wrapper function). Like setting an input, it requires
`&mut` access to the database and therefore cancels in-flight queries on other handles.

### Memory Management

LRU evicts memoized values, not query keys or dependency metadata. Salsa also
reclaims stale tracked outputs and unused low-durability interned values. Input
identities remain until the database is dropped.

## Interned structs

[Interned structs](./overview.md#interned-structs) (`#[salsa::interned]`) can make key lookup cheaper and save memory:
each distinct value is stored once, and comparing or hashing an interned struct only compares its integer ID.
Interning is especially useful for names and other small values that are compared often.
The [`calc` example](https://github.com/salsa-rs/salsa/tree/master/examples/calc) interns
function and variable names.

Interned values that have not been used for some number of revisions can be reclaimed, and their slots reused.
The `revisions` option controls how many revisions an unused value is kept for (the default is 3).
`revisions = usize::MAX` disables reclamation, which is required for interned structs that use
`unsafe(no_lifetime)` to drop the `'db` lifetime.

## Cancellation

Salsa cancels in-flight queries when their results are no longer needed:

- when another database handle requests `&mut` access, for example to set an input;
- when `db.trigger_cancellation()` is called;
- when `cancel()` is called on a token returned by `db.cancellation_token()`, which cancels only the queries running on that handle.

Each access of an intermediate query is a potential cancellation point. Cancellation is
implemented by unwinding with a `salsa::Cancelled` payload (without running the panic hook),
and Salsa internals are intended to be panic-safe. Use `salsa::Cancelled::catch` to run a
closure and turn cancellation into an `Err(Cancelled)`.

If you have a query that contains a long loop which does not execute any intermediate queries,
salsa won't be able to cancel it automatically. You may wish to check for cancellation yourself
by invoking `db.unwind_if_revision_cancelled()`.

For more details on cancellation, see the tests for cancellation behavior in the Salsa repo.
