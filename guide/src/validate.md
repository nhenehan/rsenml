# Checking packs

In this chapter you'll see what the RFC's rules refuse, and how to ask for more.

An HVAC unit reports its supply air temperature, its compressor, and its fan:

```rust,ignore
{{#include ../../examples/basics/src/bin/validate.rs:main}}
```

Run it with `cargo run --bin validate`:

```text
good pack: Ok(())
no value: Err(MissingValue { record: 0 })
unknown _ label: Err(MustUnderstand { record: 0, label: "mode_" })
RPM: Ok(())
RPM, registered units only: Err(UnregisteredUnit { record: 0 })
```

`decode` runs the same checks while it reads, and `encode` runs them before it
writes, so you only call `validate` yourself for packs you build or change.

## The rules

These come from [RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4):

- A pack has at least one record.
- A record has a value or a sum, unless it holds only base fields.
- Each record has at most one value (`v`, `vs`, `vb` or `vd`), and each label at
  most once.
- A record needs a name or a base name, and the full name must be a valid name.
- Numbers must be finite (no NaN or infinity).
- The version (`bver`) must be a positive integer, the same in every record, and
  not newer than 10.
- A label that ends in `_` and that rsenml does not know is refused.

## Units

RFC 8428 does not require registered units, so `"RPM"` passes by default.
`ValidationOptions::new().with_registered_units(true)` only allows the units of
the SenML Units registry (Table 6 of
[section 12.1](https://www.rfc-editor.org/rfc/rfc8428#section-12.1)).
`rsenml::units::lookup("Cel")` tells you what a unit means.

The registry in rsenml is RFC 8428's Table 6. The units added later by RFC 8798
are not in it yet.

## Checking a stream

For records that arrive one at a time, a `Validator` checks each record with
`check`, and `finish` checks the end of the pack.
