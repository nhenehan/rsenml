# Small devices

In this chapter you'll write a pack into a fixed buffer, as a microcontroller
would.

Every rsenml crate is `no_std`: it needs an allocator, but not the standard
library. The crates build for Cortex-M (`thumbv7em-none-eabihf`) and forbid
`unsafe` code.

A soil moisture node has a small buffer for its radio and doesn't want the output
to grow without limit:

```rust,ignore
{{#include ../../examples/basics/src/bin/fixed_buffer.rs:main}}
```

Run it with `cargo run --bin fixed_buffer`:

```text
CBOR: 73 bytes
EXI:  71 bytes
too small: Err(BufferFull)
```

`encode_into` exists for JSON, CBOR, XML and EXI. It returns the number of bytes
written, or `Error::BufferFull`.

## Making the build small

- Turn off the formats you don't use (`default-features = false`).
- Writing JSON or XML text links Rust's shortest float printing. A device that
  only sends CBOR or EXI doesn't pay for it.
- rsenml has its own decimal-to-float conversion and fixed error texts, so
  reading doesn't pull in `core`'s float parser or `format!`.

On `thumbv7em-none-eabihf` with `opt-level = "z"`, a program that decodes,
validates and encodes one pack takes about 31 KB of code and constants for CBOR,
43 KB for JSON and 129 KB for EXI (`.text` + `.rodata`, measured by `compare/`;
see [`doc/comparison.md`](https://github.com/nhenehan/rsenml/blob/main/doc/comparison.md)).
