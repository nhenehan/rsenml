# rsenml

SenML (Sensor Measurement Lists, [RFC 8428](https://www.rfc-editor.org/rfc/rfc8428))
is a small, standard way for sensors to send readings: a list of records, each
with a name, a value, and maybe a unit and a time. rsenml reads and writes SenML
in Rust. It also handles SenSML, the streaming form of SenML.

**Supported:** all four SenML representations, JSON, CBOR, XML and EXI, over one
data model. EXI is SenML's XML form coded as EXI (Efficient XML Interchange), as
RFC 8428 section 8 describes, built on [rexi](https://github.com/nhenehan/rexi).
Every crate is `no_std` (it needs only `alloc`), forbids `unsafe`, and builds for
Cortex-M (`thumbv7em-none-eabihf`).

## Quick example

```rust
use rsenml::{MediaType, Pack, Record};

fn main() -> Result<(), rsenml::Error> {
    // A pack of two records. The first sets a base name for both.
    let pack: Pack = [
        Record::new()
            .with_base_name("urn:dev:ow:10e2073a01080063:")
            .with_name("temp")
            .with_unit("Cel")
            .with_value(23.1),
        Record::new().with_name("open").with_bool_value(false),
    ]
    .into_iter()
    .collect();

    // Write it in each format, then read it back.
    let formats = [MediaType::SenmlJson, MediaType::SenmlCbor, MediaType::SenmlXml, MediaType::SenmlExi];
    for media in formats {
        let bytes = rsenml::encode(media, &pack)?;
        assert_eq!(rsenml::decode(media, &bytes)?, pack);
        println!("{media:?}: {} bytes", bytes.len());
    }

    // Resolve the pack: full names and absolute times (RFC 8428 section 4.6).
    for record in pack.resolve(Some(1_700_000_000.0))? {
        println!("{} = {:?}", record.name, record.value);
    }
    Ok(())
}
```

It prints:

```text
SenmlJson: 93 bytes
SenmlCbor: 63 bytes
SenmlXml: 150 bytes
SenmlExi: 55 bytes
urn:dev:ow:10e2073a01080063:temp = Some(Number(23.1))
urn:dev:ow:10e2073a01080063:open = Some(Bool(false))
```

## Add to your project

```toml
[dependencies]
rsenml = "0.9"
```

Each format is a feature: `json`, `cbor`, `xml` and `exi`, all on by default. To
keep only some, turn off the defaults:

```toml
rsenml = { version = "0.9", default-features = false, features = ["cbor"] }
```

rsenml needs Rust 1.88 or newer.

## What it does

- Reads and writes all eight SenML media types: `application/senml+json`,
  `senml+cbor`, `senml+xml`, `senml-exi`, and their SenSML twins. It also knows
  their CoAP Content-Format numbers.
- Reads whole packs (`decode`) or one record at a time (`RecordReader`). Writes
  whole packs (`encode`), also into a fixed buffer (`encode_into`), or one record
  at a time (`StreamEncoder`). JSON and CBOR can also read SenSML streams as the
  bytes arrive (`StreamDecoder`).
- Checks packs against the RFC's rules (`Pack::validate`), and resolves them into
  full names and absolute times (`Pack::resolve`).
- Includes the RFC 8428 units registry and `rec=` fragment identifiers.
- Matches the RFC byte for byte. Its CBOR example and both EXI examples
  (bit-packed and byte-aligned) come out exactly the same, in both directions.
- Writes the same EXI bytes as EXIficient 1.0.7 on every test case.
- Does not panic on cut-off or changed input. Every decoder is fuzzed.

Where the RFC is unclear, the choices rsenml made are in
[doc/design.md](https://github.com/nhenehan/rsenml/blob/main/doc/design.md). One known limit: EXI cannot carry -0.0.

## Examples

- [`examples/coap-todo`](https://github.com/nhenehan/rsenml/blob/main/examples/coap-todo/README.md) is a todo list served
  over CoAP with SenML EXI messages. The server uses rsenml. The four clients are
  in Rust (rsenml), Java (Californium and EXIficient), C (libcoap and exip) and C#
  (Com.AugustCellars.CoAP and Nagasena for .NET). Run `./build.sh`, then
  `./demo.sh`.

## License

BSD-3-Clause, see [LICENSE](https://github.com/nhenehan/rsenml/blob/main/LICENSE). The downloaded standards and third
parties keep their own licenses.

## For maintainers

### Status

- All of the RFC's examples, read from the RFC text at test time, decode and
  validate. Its CBOR dump (section 6) and both EXI dumps (section 8, bit-packed and
  byte-aligned) are reproduced **byte for byte** in both directions, and the EXI
  size of the section 5.1.3 example is Table 3's 161 bytes.
- EXI output is byte-identical to EXIficient 1.0.7 on all 32 test streams.
- Every case round-trips through all eight media types. Decoders do not panic on
  cut-off or changed input.
- The readings of unclear points of the RFC, the library choices and the known
  limits are in [doc/design.md](https://github.com/nhenehan/rsenml/blob/main/doc/design.md). The requirement-by-requirement
  checklist is in [doc/conformance.md](https://github.com/nhenehan/rsenml/blob/main/doc/conformance.md). The other Rust SenML
  and EXI libraries are surveyed in [doc/landscape.md](https://github.com/nhenehan/rsenml/blob/main/doc/landscape.md) and
  measured in [doc/comparison.md](https://github.com/nhenehan/rsenml/blob/main/doc/comparison.md).

### Crates

| Crate | Representation | Built on |
|---|---|---|
| `rsenml` | all of the below as features (`json`, `cbor`, `xml`, `exi`), `decode`/`encode` by media type | |
| `rsenml-core` | data model, validation, resolution, units registry, `rec=` fragments, media types | nothing |
| `rsenml-json` | `application/senml+json`, `sensml+json` | [nanojson](https://crates.io/crates/nanojson) |
| `rsenml-cbor` | `application/senml+cbor`, `sensml+cbor` | [minicbor](https://crates.io/crates/minicbor) |
| `rsenml-xml` | `application/senml+xml`, `sensml+xml` | [xmlparser](https://crates.io/crates/xmlparser) |
| `rsenml-exi` | `application/senml-exi`, `sensml-exi` | [rexi-core](https://crates.io/crates/rexi-core) (`no_std`) |
| `rsenml-tests` | RFC vectors, round trips, EXIficient bytes, robustness (not published) | |

Each representation reads whole packs (`decode`, `decode_unchecked`), record by
record (`RecordReader`), and writes whole packs (`encode`, `encode_unchecked`, and
`encode_into` a fixed buffer), and writes SenSML a record at a time
(`StreamEncoder`). JSON and CBOR also decode SenSML streams as their bytes arrive
(`StreamDecoder`).

### Building

The build needs only crates.io. The tools that write and check generated files
(rexi's schema generator, exip's EXIficient oracle) are pinned in
`third-party.lock` and checked out outside the tree:

```bash
scripts/fetch-third-party.sh          # check out the pinned rexi (schema generator) and exip oracle
scripts/fetch-standards.sh            # download RFC 8428 (the tests read its examples)
scripts/gate.sh                       # fmt, clippy, tests, no_std builds, generated files
scripts/gate.sh --quick               # the same without the EXIficient check (no Java)
```

| Script | Purpose |
|---|---|
| `scripts/fetch-third-party.sh [--check \| --update NAME]` | pinned tools (rexi's schema generator, exip's oracle) in `~/.cache/rsenml/third_party` |
| `scripts/fetch-standards.sh [--check]` | [RFC 8428](https://www.rfc-editor.org/rfc/rfc8428) from rfc-editor.org, and its section 8 XSD taken out of it, checked by sha256 (`standards.lock`), in `~/.cache/rsenml/standards` |
| `scripts/gen-tables.sh [--check]` | EXI grammar tables of the [section 8 XSD](https://www.rfc-editor.org/rfc/rfc8428#section-8) (rexi-schema-gen) |
| `scripts/gen-exi-fixtures.sh [--check]` | EXIficient 1.0.7 reference bytes for every test case (exip's `utils/xsd2exip` oracle; Java, Maven) |
| `scripts/gate.sh [--quick]` | everything a commit must pass |
| `fuzz/run.sh [seconds]` | every fuzz target for a fixed time, 60 s each by default (nightly Rust, `cargo install cargo-fuzz`) |

Standards are linked, not committed. The tests read their vectors from the RFC
text itself, which `scripts/fetch-standards.sh` downloads from rfc-editor.org and
checks against `standards.lock`.

`examples/`, `compare/` and `fuzz/` are separate workspaces. They are built only
on request, never by the workspace or the gate.

### Releasing

1. Bump the version in `[workspace.package]` and in the rsenml entries of
   `[workspace.dependencies]` in `Cargo.toml`, and add a `## X.Y.Z - YYYY-MM-DD`
   section to `CHANGELOG.md`.
2. Push to `main` and wait for CI (`.github/workflows/ci.yml`, the same checks as
   `scripts/gate.sh`) to pass.
3. Push the tag `vX.Y.Z`. The release workflow (`.github/workflows/release.yml`)
   runs CI again, checks that the tag matches the version and that the crates
   package, and then waits for an approval of the `crates-io` environment on the
   run's page. After the approval it publishes every crate to crates.io and makes
   the GitHub Release from the CHANGELOG section.
4. If crates.io stops the publish partway (for example its limit on new crates),
   use **Re-run failed jobs**: crates already on crates.io at that version are
   skipped.

Set up the `crates-io` environment once, before the first tag: you as the required
reviewer, tags matching `v*` only, and the `CARGO_REGISTRY_TOKEN` secret in the
environment (not the repository).
