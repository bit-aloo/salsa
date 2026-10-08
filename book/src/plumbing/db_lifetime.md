# The `'db` lifetime

[Tracked](./tracked_structs.md) and interned structs are both declared with a `'db` lifetime.
This lifetime is linked to the `db: &DB` reference used to create them.
The `'db` lifetime has several implications:

* It ensures that the user does not create a new salsa revision while a tracked/interned struct is in active use. Creating a new salsa revision requires modifying an input which requires an `&mut DB` reference, therefore it cannot occur during `'db`.
    * The struct may not even exist in the new salsa revision so allowing access would be confusing.
* It lets field getters return `&'db` references directly into Salsa's storage, without copying the field values and without holding a lock for the duration of the borrow.

This section discusses the unsafe code behind field access along with the reasoning behind it. To be concrete, we'll focus on tracked structs -- interned structs are very similar.

## A note on UB

When we say in this page "users cannot do X", we mean without Undefined Behavior (e.g., by transmuting integers around etc).

## The user type is an id

The `#[salsa::tracked]` macro creates a user-exposed struct that looks roughly like this:

```rust,ignore
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct MyTrackedStruct<'db>(salsa::Id, PhantomData<fn() -> &'db ()>);
```

The `salsa::Id` identifies a slot in Salsa's paged table, where the struct's
[`Value`](./tracked_structs.md#each-tracked-struct-has-a-value-storing-its-data) lives.
The `Id` contains both the index of the slot and a *generation*.
The `PhantomData` keeps the `'db` lifetime alive without storing a reference.

Storing an id rather than a reference matters because the data outlives any particular `&'db DB` borrow:

```rust,ignore
let mut db = MyDatabase::default();
let input = MyInput::new(&db, ...);

// Revision 1:
let result1 = tracked_fn(&db, input);

// Revision 2:
input.set_field(&mut db).to(...);
let result2 = tracked_fn(&db, input);
```

Tracked structs created by `tracked_fn` during Revision 1
may be reused during Revision 2, but the original `&db` reference
used to create them has expired.
Whenever users invoke the getter for a field,
Salsa looks up the `Value` in the table and creates a new reference to its contents,
tied to the `db` reference passed to the getter:

```rust,ignore
impl<'db> MyTrackedStruct<'db> {
    fn field(self, db: &'db dyn DB) -> &'db FieldType {
        ...
    }
}
```

## Life cycle of a tracked struct

Here is a typical sequence of operations for a tracked struct, along with the points where Salsa relies on unsafe code:

* A tracked function `f` executes in revision R0 and creates a tracked struct with identity fields `K` for the first time.
    * Salsa computes the struct's identity from `K` and the [disambiguator](./tracked_structs.md#each-tracked-struct-has-an-id), and allocates a fresh slot in the table (or reuses a slot from the free list, see below).
    * The struct's `Value` records `updated_at = Some(R0)`.
* The value of the field `field` is accessed on the tracked struct instance `ts` by invoking the method `ts.field(db)`.
    * *Unsafe:* This returns a reference into the `Value` stored in the table.
* A new revision R1 begins.
* The tracked function `f` does not re-execute in R1.
* The value of `field` is accessed again with `ts.field(db)`.
    * *Unsafe:* As before, this returns a reference into the table.
* A new revision R2 begins.
* The tracked function `f` re-executes in R2 and again creates a tracked struct with identity `K`, but with (some) different field values.
    * Salsa reuses the existing slot, updating the fields in place.
* A new revision R3 begins.
* When `f` executes this time it does NOT create a tracked struct with identity `K`.
    * Salsa deletes the struct: it clears the struct's memos and pushes its id onto the ingredient's free list.
    * The slot is not freed. When a later tracked struct reuses the slot, it increments the generation in the `Id`, so the stale `ts` is no longer equal to the new struct's id, and queries that depended on `ts` see it as changed.

Field access returns a `&`-reference into the slot, so we must ensure Rust's two core constraints are satisfied for the lifetime of that reference:

* The slot's memory will not be freed.
* The contents of the fields will not be mutated.

## Why field references remain valid

The table's pages are never freed while the database is alive, so the first constraint always holds.
The second constraint is maintained by the `updated_at` field of the `Value`, which acts as a per-revision read/write lock:

* Reading a field in revision R sets `updated_at` to `Some(R)` (if it was older).
  From then on, the fields are considered *read-locked* for the rest of the revision.
* Updating the fields in place (when the creating query re-executes) or deleting the struct requires swapping `updated_at` to `None`, which acts as the write lock.
  Salsa only does this if `updated_at` holds an *older* revision.
  If the struct was already created or read in the current revision, Salsa reuses the existing fields (when recreating it) or panics (when deleting it), rather than mutating fields that may be borrowed.

This works because of the `'db` lifetime:

* **No forgery.** Salsa's public API never turns an arbitrary id into a tracked struct, so every `TS<'db>` originates from Salsa, either from `TS::new` or from a memoized value. (The struct's tuple fields are private, but privacy is per module, so code in the module that declares the struct must not construct one by hand.)
* **Within one revision.** The `'db` lifetime of `ts: TS<'db>` comes from a `db: &'db dyn Db` borrow. Beginning a new revision requires `&mut` access to the database, so as long as users hold `ts` (or a reference to one of its fields), no new revision can start and no fields can be updated or deleted.
* **The creating query ran first.** To obtain `ts` in revision R, the creating query `f` must either have executed in R or been validated in R, because `ts` can only flow from `f`'s result (or from tracked structs and memoized values that depend on it). When `f` executes, it updates the struct before handing it out, so no reader can observe the fields mid-update.

The remaining hazard is a value with a `'db` lifetime that is stored in Salsa and read in a later revision, after the borrow that produced it has ended.
That is what the next section is about.

## The `'db` lifetime at rest

Salsa stores tracked and interned fields and memoized query results after the particular `&'db DB`
borrow that produced them has ended. Internally, Salsa erases that lifetime while the value is in
storage and restores the current database lifetime when the value is accessed again. The unsafe
ingredient `Configuration` traits guarantee that their `Output<'db>` or `Fields<'db>` associated
types can be stored with `'db` replaced by `'static` and later restored. The generated unsafe
implementations are justified by checking that every lifetime-dependent value implements
`SalsaValue`. This is the boundary that makes rebranding sound: an older value must remain safe to
retain and use in a later revision, including through safe operations such as `PartialEq`.

`#[derive(salsa::SalsaValue)]` checks the guarantee structurally. A field whose type is
unconditionally `'static` is accepted directly. A field that borrows for `'db`, or is otherwise
not `'static`, must implement `SalsaValue`. A direct database-lifetime reference such as `&'db T`
does not implement the trait because its referent may be changed or freed in a later revision.
Salsa handles do implement it because access to their data goes back through the current database
state.

The derive supports types with at most one lifetime parameter, as well as type and const
parameters. Generated implementations require generic field types to implement `SalsaValue`.
Retention proofs are supported by `derive(SalsaValue)` and on tracked and interned struct fields;
input fields do not support them.

On a type using `derive(SalsaValue)`, conditional proofs add predicates to the generated
`SalsaValue` implementation, narrowing the generic instantiations that implement the trait. Tracked
and interned structs do not support type or const parameters, so their predicates are instead
verified for every database lifetime by the generated field assertions. When an unmodifiable field
type cannot implement `SalsaValue`, a conditional proof can replace the field check with narrower
predicates:

```rust,ignore
#[derive(salsa::SalsaValue)]
struct QueryValue<T> {
    #[salsa_value(unsafe(prove(T: salsa::SalsaValue)))]
    value: ForeignContainer<T>,
}
```

The predicates are added to the generated implementation, so `QueryValue<T>` only implements
`SalsaValue` when `T` does. The compiler verifies that premise; the author asserts that the premise
implies `ForeignContainer<T>` remains valid when Salsa retains it across revisions and rebinds its
database lifetime.

An unconditional `#[salsa_value(unsafe(prove_safe_to_retain_manually))]` proof skips the structural
check without adding predicates. The author must ensure the field is safe for every generic
instantiation accepted by the enclosing type.

## Ways this could go wrong

These are the main ways that user code could break the reasoning above, and how Salsa prevents them:

* **Storing an `&'db T` in a field or query result.** A direct reference into the database could outlive the revision it was created in. `SalsaValue` is not implemented for `&'db T`, so such values are rejected, as described above.
* **Leaking a tracked struct across threads or revisions with unsafe code.** If a write lock cannot be acquired because the struct is in use, Salsa panics instead of mutating fields that may be borrowed.
* **Non-deterministic tracked functions.** If a query creates different structs when it is re-executed in the same revision, Salsa may try to delete a struct that was already read in the current revision. That also panics rather than causing undefined behavior.
