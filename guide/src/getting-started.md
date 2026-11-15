# Getting started

In this chapter you'll add rsenml to your project and run the examples.

## Add rsenml to your project

```toml
[dependencies]
rsenml = "0.9"
```

Each format is a feature: `json`, `cbor`, `xml` and `exi`, all on by default. If
your device only sends CBOR, keep just that one:

```toml
[dependencies]
rsenml = { version = "0.9", default-features = false, features = ["cbor"] }
```

The data model (records, packs, validation, resolving, units, media types) is
always there. Each format is also its own crate (`rsenml-json`, `rsenml-cbor`,
`rsenml-xml`, `rsenml-exi`), if you prefer to depend on one directly.

rsenml needs Rust 1.88 or newer.

## Run the examples

The examples live in `examples/basics` and build on their own:

```bash
git clone https://github.com/nhenehan/rsenml
cd rsenml/examples/basics
cargo run --bin first_pack
./run-all.sh          # every example, in the order of this guide
```

## The calls you'll use most

| Task | Call |
|---|---|
| Build a record | `Record::new().with_name("room").with_unit("Cel").with_value(21.5)` |
| Build a pack | `[r1, r2].into_iter().collect::<Pack>()` |
| Write one format | `rsenml::json::encode(&pack)` (also `cbor`, `xml`, `exi`) |
| Read one format | `rsenml::json::decode(&bytes)` |
| Pick the format at run time | `rsenml::encode(media, &pack)`, `rsenml::decode(media, &bytes)` |
| Check a pack | `pack.validate()` |
| Full names and times | `pack.resolve(Some(now))` |

`decode` checks the pack as it reads, and `encode` checks it before writing. The
`_unchecked` versions skip that, for packs you know are valid.
