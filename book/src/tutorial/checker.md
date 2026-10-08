# Defining the checker

With the parser in place, the next step is the **checker**.
Its job is to make sure that the program only references names that exist:
every variable used in a function body must be one of that function's parameters,
and every function that is called must be defined somewhere in the program.
It reports a `Diagnostic` for each violation, using the same [accumulator](./accumulators.md) as the parser.

The checker is in the [`type_check`] module.
Like the parser, it is mostly ordinary Rust code;
the interesting part is deciding which functions should be tracked, because that determines how much work Salsa can reuse when the program changes.

[`type_check`]: https://github.com/salsa-rs/salsa/blob/master/examples/calc/type_check.rs

## The tracked entry points

The checker is split into two tracked functions: one for the whole program and one for each function definition.

```rust
{{#include ../../../examples/calc/type_check.rs:type_check_program}}
```

`type_check_program` walks the program's statements.
It checks `print` expressions itself, with no names in scope, but it hands each function definition to `type_check_function`.

Neither function returns anything: their only output is the diagnostics they accumulate.
They are tracked anyway, so that Salsa memoizes the diagnostics.
When a tracked function is reused, the values it accumulated are reused too.

### Why check each function separately?

Making `type_check_function` a separate tracked function is what makes the checker incremental.
Recall from the [IR chapter](./ir.md#representing-functions) that `Function` is a tracked struct whose `args` and `body` are `#[tracked]` fields.
`type_check_function` only reads the `args` and `body` of the function it is given,
so Salsa records dependencies on exactly those two fields.
If the user edits the body of one function, only that function is checked again;
the memoized results (and diagnostics) for every other function are reused.

Notice that `type_check_function` also takes the `Program` as a second argument.
It needs the program to resolve calls to other functions, as we'll see below.
Tracked functions with more than one Salsa struct argument are allowed:
Salsa interns the `(function, program)` tuple to create the key it memoizes on.

## Checking expressions

The recursive walk over expressions doesn't need to be tracked:
expressions aren't Salsa structs, and all the expressions in a function body belong to that function,
so tracking `type_check_function` is enough.
The walk is a plain Rust struct that carries the database, the program, and the names currently in scope:

```rust
{{#include ../../../examples/calc/type_check.rs:check_expression}}
```

Errors are reported in the same way as in the parser, by accumulating a `Diagnostic`:

```rust
{{#include ../../../examples/calc/type_check.rs:report_error}}
```

`Span` is a tracked struct, so `span.start(self.db)` and `span.end(self.db)` are reads of tracked fields.
They create dependencies just like reading `function.body(db)` does.

## Looking up functions

When the checker sees a call such as `area_circle(1)`, it has to find the definition of `area_circle`.
That lookup is itself a tracked function:

```rust
{{#include ../../../examples/calc/type_check.rs:find_function}}
```

`find_function` reads `program.statements`, but it returns only an `Option<Function>`.
That makes it a "firewall".
Suppose the user edits the program so that `statements` changes, but `area_circle` is still defined by the same `Function`.
Then `find_function` re-executes, returns the same value as before, and Salsa [backdates](../plumbing/terminology/backdate.md) the result.
Any query that depended on `find_function` sees no change, so it doesn't need to re-execute.
If `type_check_function` read `program.statements` directly instead,
every edit anywhere in the program would cause every function to be checked again.

`Function` values can be compared across revisions like this because tracked structs have a stable identity.
The parser recreates every `Function` each time it runs,
but a `Function` created by the same tracked function with the same values for its identity fields (`name` and `name_span`) gets the same ID as before.
See [identity fields and tracked fields](./ir.md#identity-fields-and-tracked-fields).

## Testing the checker

The tests use a helper, `check_string`, that parses and checks a string and then renders the accumulated diagnostics.
To read the diagnostics, it calls `type_check_program::accumulated::<Diagnostic>`.
This collects the diagnostics from `type_check_program` and from every tracked function it called, including each `type_check_function`:

```rust
let diagnostics = type_check_program::accumulated::<Diagnostic>(db, program);
```

For example, this test checks a program that uses two undeclared variables:

```rust
{{#include ../../../examples/calc/type_check.rs:check_bad_variable_in_program}}
```

### Observing reuse

We can also check that Salsa really avoids re-checking functions that didn't change.
The test database records a log entry each time Salsa is about to execute a tracked function
(see the `WillExecute` event in the [database chapter](./db.md)).
This test edits the body of `double` and then looks at which functions re-executed:

```rust
{{#include ../../../examples/calc/type_check.rs:check_reuse}}
```

Only two functions re-execute: the parser, because the source text changed, and `type_check_function` for `double`.
Here is why the rest are reused:

- The new `statements` vector is equal to the old one.
  The edit only changed the body of `double`, which lives in a tracked field of the `Function` struct, and the vector holds only the `Function`'s ID.
  So `program.statements` is backdated,
  and neither `type_check_program` nor `find_function` needs to re-execute.
- Before reusing `type_check_program`, Salsa makes sure that none of the functions it called have changed.
  Salsa finds that `type_check_function(double)` read the `body` of `double`, which did change, so it re-executes that function.
  It returns `()` again, so its result counts as unchanged and `type_check_program` can still be reused.
  When we later ask for the accumulated diagnostics, Salsa collects them from the new run of `type_check_function(double)`.
- `type_check_function(quadruple)` read only the `args` and `body` of `quadruple`, plus the result of `find_function`.
  None of those changed, so it is reused.
