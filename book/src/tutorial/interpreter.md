# Defining the interpreter

The last piece of `calc` is the **interpreter**, which runs the program and produces its output.
Once the interpreter is done, we put the parser, checker, and interpreter together in a small driver.

The interpreter is in the [`interpret`] module.

[`interpret`]: https://github.com/salsa-rs/salsa/blob/master/examples/calc/interpret.rs

## The `interpret_program` function

The interpreter has a single tracked function, `interpret_program`.
It evaluates each `print` statement and returns the printed lines:

```rust
{{#include ../../../examples/calc/interpret.rs:interpret_program}}
```

Like any tracked function, `interpret_program` must be deterministic and free of side effects,
so it doesn't print anything itself.
It returns the output as a `Vec<String>` and leaves printing to the driver.
That also makes the output easy to test.

The function is annotated `#[salsa::tracked(returns(deref))]`.
As described in the [parser chapter](./parser.md#the-returnscopy-annotation),
this makes the function return a `&[String]` that borrows from the memoized `Vec<String>`,
instead of a `&Vec<String>`.

## Evaluating expressions

Evaluation is a plain recursive walk over the expression tree.
It doesn't involve any Salsa structs of its own:

```rust
{{#include ../../../examples/calc/interpret.rs:interpreter}}
```

There are a few things to note:

- The interpreter reuses the checker's tracked `find_function` query to resolve calls.
  Tracked functions can be shared freely across the phases of a compiler:
  the interpreter and checker both use the same memoized lookup.
- Errors are reported as values (`Result<f64, String>`) rather than with an accumulator.
  The driver only runs the interpreter if the checker found no errors,
  but the checker doesn't catch everything:
  calling a function with the wrong number of arguments, or recursing forever, are only caught at runtime.
  The error messages are part of the program's output, so it's natural to return them.
- `calc` has no conditionals, so a recursive function can never terminate.
  The interpreter stops at a fixed call depth and reports an error.
  We can't let Salsa detect this as a [cycle](../cycles.md),
  because evaluating a call isn't a tracked function, and each recursive call has different arguments anyway.

## How much does the interpreter reuse?

We deliberately made the whole program the unit of reuse for the interpreter.
`interpret_program` reads `program.statements`, so it re-executes whenever the statements change.
Evaluating `calc` programs is cheap, and finer-grained tracking would cost more than it saves.

`interpret_program` still benefits from [backdating](../plumbing/terminology/backdate.md), though.
If an edit to the source text doesn't change the parsed statements (for example, adding whitespace at the end of the file),
then the parser re-executes but produces an equal `Program`, and the interpreter's output is reused:

```rust
{{#include ../../../examples/calc/interpret.rs:interpret_reuse}}
```

If evaluation were expensive, we could make it more incremental.
For example, we could add a tracked function `interpret_function(db, function, args)` that evaluates one call and memoizes the result.
That would apply the same idea as the checker's `type_check_function`.
Try it as an exercise!

## Putting it all together

Finally, we need a driver.
The driver is the only part of the program that owns the database mutably,
so it is the one that creates inputs, changes them, and reads results.

First, a tracked `compile` function runs the parser and the checker:

```rust
{{#include ../../../examples/calc/compile.rs:compile}}
```

Calling `compile::accumulated::<Diagnostic>` collects every diagnostic produced while compiling,
from both `parse_statements` and the checker.
That works because `compile` called both of them.

The `main` function creates the database and the `SourceProgram` input, then runs the program.
After the first run, it edits the input and runs it again:

```rust
{{#include ../../../examples/calc/main.rs:main}}
```

The second call to `run` happens in a new revision, because `set_text` changed an input.
Salsa re-executes the parser, because the text changed.
The new `print` statement changes `program.statements`, so `type_check_program`, `find_function`, and `interpret_program` re-execute.
The two function definitions didn't change, though, so their `type_check_function` results are reused.

Run the example with `cargo run --example calc`.
It prints:

```text
12
3.14
22
--- after edit ---
12
3.14
22
12.56
```

You can also give it the path of a `calc` program, for example `cargo run --example calc -- my_program.calc`.
If the program has errors, the driver prints the diagnostics instead of running it:

```text
error: the variable `b` is not declared
 --> input:1:15
  |
1 | fn f(a) = a + b
  |               ^ here
```

That's the end of the tutorial!
To go further, the [reference](../reference.md) and [common patterns](../common_patterns.md) chapters cover more Salsa features.
