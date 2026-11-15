# When things go wrong

In this chapter you'll see what the errors look like.

Every call returns `rsenml::Result<T>`, with one error type, `rsenml::Error`, for
all formats:

```rust,ignore
{{#include ../../examples/basics/src/bin/errors.rs:main}}
```

Run it with `cargo run --bin errors`:

```text
cut off: JSON input ends inside the pack
    Incomplete { format: Json }
two values: record 0: more than one of v, vs, vb, vd
    MultipleValues { record: 0 }
bad name: record 0: name must be [A-Za-z0-9][A-Za-z0-9:./_-]* (RFC 8428 4.5.1)
    InvalidName { record: 0 }
newer version: record 0: SenML version 11 is newer than 10 and must not be used
    UnsupportedVersion { record: 0, version: 11 }
```

The `Display` text is for people; match on the variant in code. Errors about one
record say which one (`record`, counted from 0).

| Error | Meaning |
|---|---|
| `Syntax` | The bytes are not valid JSON, CBOR, XML or EXI. It gives the byte offset. |
| `Incomplete` | The input ended too early. For a stream: wait for more bytes. |
| `WrongType`, `DuplicateLabel`, `MultipleValues` | A record's fields are wrong. |
| `MustUnderstand` | A label ending in `_` that rsenml doesn't know. |
| `InvalidLabel` | An extension uses a name the format keeps for a registered label. |
| `InvalidBase64`, `NonFinite` | A data value or a number that SenML can't carry. |
| `EmptyPack`, `MissingValue`, `MissingName`, `InvalidName` | The pack breaks a rule of section 4. |
| `InvalidVersion`, `UnsupportedVersion`, `InconsistentVersion` | `bver` problems. |
| `UnregisteredUnit` | Only with `registered_units` turned on. |
| `RelativeTime` | Resolving found a relative time, and no "now" was given. |
| `Structure` | The XML or EXI root or record element is missing or wrong. |
| `SchemaId` | The EXI stream doesn't name the RFC 8428 schema (`"a"`). |
| `Unrepresentable` | The pack can't be written in this format (for example, a feature that is off). |
| `BufferFull` | `encode_into` ran out of room. |
| `Exi` | An error from the EXI codec that has no SenML meaning. |

The full list, with every field, is on
[docs.rs](https://docs.rs/rsenml-core/latest/rsenml_core/enum.Error.html).
