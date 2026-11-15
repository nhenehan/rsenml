# Changelog

## 0.9.2 - 2026-11-15

- **On crates.io.** `rsenml = "0.9"`. rsenml-exi now uses `rexi-core` 0.3 from
  crates.io instead of rexi's git tag `v0.2.0`, and its grammar tables are
  regenerated with rexi-schema-gen 0.3.0. rexi stays out of rsenml's API, so
  nothing changes for callers.
- **CI and releases** on GitHub Actions: `ci.yml` runs the checks of
  `scripts/gate.sh`; `release.yml` publishes a version tag to crates.io after an
  approval.
- **Fix: a base-only record with an unknown label** (`[{"bn":"a:","bt":1,"ver":2}, …]`)
  was refused with "neither a value nor a sum". Readers ignore unknown labels
  ([RFC 8428 section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)),
  so `Record::has_regular_fields` no longer counts extensions, and the record is
  base-only again. A label ending in `_` is still refused. Found by
  rsenml-interop with cisco/senml's `data.json`; new vectors
  `j-base-only-unknown-label` and `c-base-only-unknown-key`.
- **RFC text: short cited excerpts, not whole RFCs.** `spec/` is gone (the full
  text of RFC 8428, 8798, 9100 and 9193). The tests read only the parts of RFC 8428
  they need, its examples, hex dumps and section 8 schema, from
  `crates/rsenml-tests/fixtures/rfc8428`, which cites the section of each file.
  `tests/extract.rs` checks them against the full RFC, which
  `scripts/fetch-standards.sh` downloads (checked by sha256 against
  `standards.lock`); the full gate and CI run that check.
- **Docs in a new layout**: a user guide (`guide/`, published on GitHub Pages)
  with the programs of `examples/basics`; a shorter README with feature,
  standards and conformance tables; `CONTRIBUTING.md`, `AGENTS.md` and
  `references.md`. `doc/design.md` and `doc/conformance.md` moved into the guide
  and CONTRIBUTING.md.
- **Exported test data**: `scripts/export-fixtures.sh` writes the conformance
  vectors and the test cases, with rsenml's verdicts and bytes in every format,
  as TSV in `crates/rsenml-tests/fixtures/export` (for SenMLFormat, the Julia port). The gate
  checks that they are up to date.

## 0.9.1

- **EXI** uses rexi 0.2.0, by its `v0.2.0` tag. The grammar tables now carry
  schemaId `"a"`, so rexi itself refuses a stream that names another schema; it
  is still reported as `Error::SchemaId`. No rsenml API changes.
- **Comparison**: `doc/comparison.md` opens with a features table (what each
  library's API offers, and RFC 8428 features as measured by the test vectors)
  and ends with the timing table.
- **Third parties**: a `third-party.lock` line can name a tag; rexi is pinned by
  `v0.2.0`.

## 0.9.0

The first public release. RFC 8428 SenML and SenSML in JSON, CBOR, XML and EXI.
Every crate is `no_std` (it needs only `alloc`) and builds for Cortex-M.

- **Data model** (`rsenml-core`): `Record`, `Pack`, `Value` and `Extension`;
  validation (`Pack::validate`, `Validator`); resolution into full names and
  absolute times (`Pack::resolve`, `Resolver`); the RFC 8428 units registry;
  `rec=` fragment identifiers; media types and CoAP Content-Formats; base64url.
- **Formats**: `rsenml-json` (on nanojson), `rsenml-cbor` (on minicbor),
  `rsenml-xml` (on xmlparser) and `rsenml-exi` (on rexi; strict, schemaId "a").
  Each has `decode`, `decode_unchecked`, `RecordReader`, `encode`,
  `encode_unchecked`, `encode_into` (for a fixed buffer) and `StreamEncoder`.
  JSON, CBOR and XML also have `encode_record`.
- **Streams**: JSON and CBOR have a `StreamDecoder` for SenSML input that arrives
  in pieces. CBOR has `decode_sensml` and `encode_sensml`.
- **One crate for all**: `rsenml` has a feature per format, and `decode`,
  `encode` and `encode_unchecked` by `MediaType`.
- **Checked**: the RFC's CBOR and EXI examples come out byte for byte. EXI output
  matches EXIficient 1.0.7. Every decoder is fuzzed.
- **Docs**: the choices made where RFC 8428 is unclear are in `doc/design.md`.
- **Example**: `examples/coap-todo`, a todo list over CoAP with Rust, Java, C and
  C# clients.
- Needs Rust 1.88 or newer.
