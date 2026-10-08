# Basic structure

Before we do anything with Salsa, let's talk about the basic structure of the calc compiler.
Part of Salsa's design is that you are able to write programs that feel 'pretty close' to what a natural Rust program looks like.

## Example program

This is our example calc program:

```
fn area_rectangle(w, h) = w * h
fn area_circle(r) = 3.14 * r * r
print area_rectangle(3, 4)
print area_circle(1)
print 11 * 2
```

A program is a list of statements.
A `fn` statement defines a function whose body is a single expression over its parameters,
and a `print` statement evaluates an expression and prints the result.

## Parser

The calc compiler takes as input a program, represented by a string:

```rust
struct SourceProgram {
    text: String
}
```

The first thing it does it to parse that string into a series of statements that look something like the following pseudo-Rust:[^lexer]

```rust
enum Statement {
    /// Defines `fn <name>(<args>) = <body>`
    Function(Function),
    /// Defines `print <expr>`
    Print(Expression),
}

/// Defines `fn <name>(<args>) = <body>`
struct Function {
    name: FunctionId,
    args: Vec<VariableId>,
    body: Expression
}
```

where an expression is something like this (pseudo-Rust, because the `Expression` enum is recursive):

```rust
enum Expression {
    Op(Expression, Op, Expression),
    Number(f64),
    Variable(VariableId),
    Call(FunctionId, Vec<Expression>),
}

enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
}
```

Finally, for function/variable names, the `FunctionId` and `VariableId` types will be interned strings:

```rust
type FunctionId = /* interned string */;
type VariableId = /* interned string */;
```

[^lexer]: Because calc is so simple, we don't have to bother separating out the lexer from the parser.

## Checker

The "checker" has the job of ensuring that the user only references variables and functions that have been defined.
It checks each function definition separately, so that when one function changes,
only that function needs to be checked again.

## Interpreter

The interpreter will execute the program and print the result. We don't bother with much incremental re-use here,
though it's certainly possible.

## Driver

Finally, a small `main` function ties everything together:
it creates the database, supplies the source text, reports any errors, and prints the program's output.
