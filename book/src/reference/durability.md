# Durability

"Durability" is an optimization that can greatly improve the performance of your salsa programs.
Durability specifies the probability that an input's value will change.
The default is "low durability".
But when you set the value of an input, you can manually specify a higher durability,
typically `Durability::HIGH`.
Salsa tracks when tracked functions only consume values of high durability
and, if no high durability input has changed, it can skip traversing their
dependencies.

Typically "high durability" values are things like data read from the standard library
or other inputs that aren't actively being edited by the end user.

Salsa provides four durability levels:

- `Durability::LOW`, the default, for values that change frequently, such as the contents of files being edited;
- `Durability::MEDIUM`, for values that change occasionally;
- `Durability::HIGH`, for values that rarely change;
- `Durability::NEVER_CHANGE`, for values that can never change once set. Setting a field with this durability again panics.

To set a field with a particular durability, use `with_durability` on the setter:

```rust
input.set_text(&mut db).with_durability(salsa::Durability::HIGH).to(new_text);
```

If you don't call `with_durability`, the field keeps the durability it had before.
To choose durabilities when creating an input, use the builder:
`MyInput::builder(field_values).durability(d).new(&db)` sets the durability of all fields, and
`<field>_durability(d)` sets the durability of a single field.
