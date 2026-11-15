# rsenml basics

The programs of the [user guide](https://nhenehan.github.io/rsenml/), one per
chapter. This is its own workspace, so the rsenml workspace never builds it.

```bash
cargo run --bin first_pack
./run-all.sh          # every program, in the guide's order
```

| Program | Chapter | What it shows |
|---|---|---|
| `first_pack` | Your first pack | A room thermostat's pack, written as JSON and read back |
| `formats` | Choosing a format | The same pack in all eight media types, and byte-aligned EXI |
| `resolve` | Base fields and resolving | A water meter's sums, relative times and full names |
| `validate` | Checking packs | What the RFC's rules refuse, and registered units only |
| `stream` | Streaming with SenSML | A tank level sensor, written and read record by record |
| `fixed_buffer` | Small devices | CBOR and EXI into a fixed buffer |
| `errors` | When things go wrong | What decoding errors look like |
