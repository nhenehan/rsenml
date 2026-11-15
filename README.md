# rsenml

rsenml reads and writes sensor readings in Rust. The format is SenML (Sensor
Measurement Lists, [RFC 8428](https://www.rfc-editor.org/rfc/rfc8428)), a small
IETF standard: a list of records, each with a name, a value, and maybe a unit and
a time. A thermostat, a water meter and a web app can all agree on it without
sharing any code.

**Supported:** all four SenML formats (JSON, CBOR, XML and EXI) over one data
model, and SenSML, the streaming form. Every crate is `no_std` (it needs only
`alloc`), forbids `unsafe`, and builds for Cortex-M.

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

You can also depend on one format's crate directly (`rsenml-cbor = "0.9"` and so
on). rsenml needs Rust 1.88 or newer.

## Learn more

- **[The user guide](https://nhenehan.github.io/rsenml/)** works through one small
  program per idea, from a first pack to streaming and small devices. To read it
  offline: `mdbook-mermaid install guide && mdbook serve guide` (you need `mdbook`
  and `mdbook-mermaid`).
- **[API documentation](https://docs.rs/rsenml)** on docs.rs.
- **[`examples/basics`](https://github.com/nhenehan/rsenml/blob/main/examples/basics/README.md)**
  contains the programs from the guide.
- **[`examples/coap-todo`](https://github.com/nhenehan/rsenml/blob/main/examples/coap-todo/README.md)**
  is a todo list served over CoAP with SenML EXI messages. The server uses rsenml;
  the clients are in Rust, Java (Californium and EXIficient), C (libcoap and exip)
  and C# (Nagasena for .NET).

## SenML implementation

| Feature | Status | Notes |
|---|---|---|
| JSON (`application/senml+json`) | Supported | |
| CBOR (`application/senml+cbor`) | Supported | Integer keys of Table 4; SenSML as an indefinite-length array |
| XML (`application/senml+xml`) | Supported | UTF-8 only; a DTD is refused |
| EXI (`application/senml-exi`) | Supported | Strict, with the section 8 schema, built on [rexi](https://github.com/nhenehan/rexi) |
| EXI bit-packed and byte-aligned | Supported | Written with the options in the header (schemaId `"a"`); streams without them are read when you give the options |
| SenSML streams | Supported | All four media types |
| Reading a stream as bytes arrive | JSON and CBOR | XML and EXI read record by record from whole input |
| Validation | Supported | The rules of section 4; registered units on request |
| Resolving | Supported | Full names, absolute times, base values and sums (section 4.6) |
| Units registry | Supported | Table 6 of RFC 8428 |
| `rec=` fragment identifiers | Supported | Section 9.1 |
| CoAP Content-Formats | Supported | All eight media types |
| Extensions in EXI | Not supported | Strict EXI can't carry them: dropped by default, or refused |
| Non-strict EXI | Not supported | |
| XPointer fragment identifiers | Not supported | Section 9.2 |

One known limit: EXI cannot carry -0.0, so it is written as 0. The other choices
rsenml makes where the RFC leaves room are in the guide's
[Where the RFC is unclear](https://nhenehan.github.io/rsenml/rfc-choices.html).

## Standards

| Standard | What it is | Implemented |
|---|---|---|
| [RFC 8428](https://www.rfc-editor.org/rfc/rfc8428) | SenML itself: the data model and the JSON, CBOR, XML and EXI formats | Yes |
| [RFC 9100](https://www.rfc-editor.org/rfc/rfc9100) | Updates RFC 8428: `bver` becomes a bitmap of features | No |
| [RFC 8798](https://www.rfc-editor.org/rfc/rfc8798) | More units for the SenML Units registry, and secondary units | No |
| [RFC 9193](https://www.rfc-editor.org/rfc/rfc9193) | Fields that give the Content-Format of a data value (`ct`, `ce`) | No |
| [RFC 8790](https://www.rfc-editor.org/rfc/rfc8790) | FETCH and PATCH of SenML records over CoAP | No |

[references.md](https://github.com/nhenehan/rsenml/blob/main/references.md) lists
these and the other standards rsenml uses (JSON, CBOR, EXI, base64url, CoAP).

## Conformance and interoperability

Test results from rsenml's CI.

| Test | Result |
|---|---|
| RFC 8428's JSON examples (sections 5.1.1 to 5.1.7, 11 packs) | All read and validate; 5.1.4 is 5.1.3 resolved, exactly |
| RFC 8428's CBOR dump (section 6) | Same bytes, both ways |
| RFC 8428's EXI dumps (section 8, bit-packed and byte-aligned) | Same bytes, both ways; Table 3's 161 bytes for 5.1.3 |
| EXI compared with EXIficient 1.0.7, 16 packs, both alignments | All 32 streams have the same bytes |
| Every test pack through all eight media types | Reads back the same |
| Decoders on cut-off and randomly changed input | Errors, never a panic; every decoder is also fuzzed |

The RFC examples are read from short, cited excerpts of the RFC
(`crates/rsenml-tests/fixtures/rfc8428`), checked against the full RFC text.

[`doc/comparison.md`](https://github.com/nhenehan/rsenml/blob/main/doc/comparison.md)
compares rsenml with the other Rust SenML crates on the same test vectors: features,
verdicts, sizes, dependencies, footprint and speed.
[SenMLFormat](https://github.com/nhenehan/SenMLFormat.jl), the Julia version of
rsenml, writes the same bytes and gives the same verdicts.

## Rust library features

| Feature | Crate | Description |
|---|---|---|
| One dependency | `rsenml` | Re-exports the crates below by feature; `decode` and `encode` by media type |
| Data model | `rsenml-core` | Records, packs, validation, resolving, units, `rec=` fragments, media types |
| JSON | `rsenml-json` | On [nanojson](https://crates.io/crates/nanojson) |
| CBOR | `rsenml-cbor` | On [minicbor](https://crates.io/crates/minicbor) |
| XML | `rsenml-xml` | On [xmlparser](https://crates.io/crates/xmlparser) |
| EXI | `rsenml-exi` | On [rexi-core](https://crates.io/crates/rexi-core) |
| Record by record | every format | `RecordReader` and `StreamEncoder`; `StreamDecoder` for JSON and CBOR |
| Fixed-buffer output | every format | `encode_into` |
| `no_std` | every crate | Needs only `alloc` |

## Safe with untrusted data

A few hostile bytes can ask a decoder for a lot of work, such as an array of 2<sup>64</sup>
records or endless nesting. rsenml's decoders return an error instead: deep nesting
stops at a fixed depth, huge announced lengths fail cleanly, and XML DTDs (and
so entity expansion) are refused. `decode` also validates the pack, so what you
get back follows RFC 8428's rules.

See the guide's [Untrusted input](https://nhenehan.github.io/rsenml/untrusted-input.html) chapter.

## Thanks

- **Cullen Jennings, Zach Shelby, Jari Arkko, Ari Keränen and Carsten Bormann**
  wrote RFC 8428. Its examples, hex dumps and schema are the first tests rsenml
  must pass.
- **Daniel Peintner** wrote [EXIficient](https://exificient.github.io/) with
  Richard Kuntschke at Siemens. rsenml's EXI bytes are checked against EXIficient's.
- **Toralf Wittner** ([minicbor](https://crates.io/crates/minicbor)), **Yevhenii
  Reizner** ([xmlparser](https://crates.io/crates/xmlparser)) and **Gronis**
  ([nanojson](https://crates.io/crates/nanojson)) wrote the small `no_std` parsers
  rsenml is built on.

Thank you.

## License

BSD-3-Clause, see [LICENSE](https://github.com/nhenehan/rsenml/blob/main/LICENSE).
The RFC 8428 excerpts in `crates/rsenml-tests/fixtures/rfc8428` keep the IETF
Trust's license (see the README there).

For build instructions, tests, generated files and other development details, see
[CONTRIBUTING.md](https://github.com/nhenehan/rsenml/blob/main/CONTRIBUTING.md).
