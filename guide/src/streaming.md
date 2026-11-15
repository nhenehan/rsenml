# Streaming with SenSML

In this chapter you'll write records one at a time and read them back as their
bytes arrive.

A tank level sensor sends a reading every minute over a connection that stays
open. The writer doesn't know how many records there will be, and the reader
wants each one as soon as it comes:

```rust,ignore
{{#include ../../examples/basics/src/bin/stream.rs:main}}
```

Run it with `cargo run --bin stream`:

```text
[{"bn":"urn:dev:mac:0024befffe804ff4:tank:","bt":1700000000,"n":"level","u":"m","v":1.82},{"n":"level","t":60,"v":1.81},{"n":"level","t":120,"v":1.79}]
got Some("level") = Some(Number(1.82))
got Some("level") = Some(Number(1.81))
got Some("level") = Some(Number(1.79))
stream finished: true
```

`push` adds bytes, and `next_record` returns a record once all of it has arrived,
or `None` to say "wait for more". Each record is checked as it comes out, so a bad
record is reported where it is, not at the end of the stream.

## What each format offers

| | JSON | CBOR | XML | EXI |
|---|---|---|---|---|
| `StreamEncoder` (write record by record) | yes | yes | yes | yes |
| `RecordReader` (read record by record, whole input) | yes | yes | yes | yes |
| `StreamDecoder` (read as bytes arrive) | yes | yes | no | no |

The CBOR `StreamEncoder` writes an array with no length up front, as SenSML in
CBOR should. The EXI `StreamEncoder` writes into any `Sink`: a `Vec<u8>`, or a
fixed `SliceBuf`.

`RecordReader` is for a whole message that you don't want to turn into a `Pack`
first, for example to look at each record and keep only some.
