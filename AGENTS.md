# AGENTS.md

## About this project

rsenml implements SenML (Sensor Measurement Lists, RFC 8428) in Rust: one data
model and four formats, JSON, CBOR, XML and EXI, plus SenSML streams.

Every library crate is `no_std` with `alloc` and forbids `unsafe`. Use the crate
that provides what you need rather than adding another JSON, CBOR, XML or EXI
layer.

## Crate selection

| If you need to... | Use |
|---|---|
| Work with records and packs, validate, resolve, look up units, parse `rec=` | `rsenml-core` |
| Read or write JSON | `rsenml-json` |
| Read or write CBOR | `rsenml-cbor` |
| Read or write XML | `rsenml-xml` |
| Read or write EXI | `rsenml-exi` |
| One dependency, or the format chosen by media type | `rsenml` |

The `rsenml` crate is the facade. It re-exports `rsenml-core` and has one feature
per format (`json`, `cbor`, `xml`, `exi`, all on by default).

## Choosing an interface

- **Whole packs:** `decode` and `encode` in each format module, or
  `rsenml::decode(media, bytes)` and `rsenml::encode(media, &pack)`. Both
  validate. The `_unchecked` versions skip validation, for packs already known to
  be valid.
- **Fixed buffer:** `encode_into` in every format.
- **Record by record from whole input:** `RecordReader` in every format.
- **Writing a stream:** `StreamEncoder` in every format (CBOR writes an
  indefinite-length array; EXI writes into a `Sink`).
- **Reading a stream as bytes arrive:** `StreamDecoder`, JSON and CBOR only.
- **Validation and resolving:** `Pack::validate`, `Pack::validate_with`,
  `Validator` (record by record), `Pack::resolve`, `Resolver` (record by record).
- **EXI options:** `exi::EncodeOptions` (alignment, cookie, extensions) and
  `exi::DecodeOptions` (out-of-band options).

## SenML rules

- Follow RFC 8428. Where it is unclear, follow the readings in the guide's
  `guide/src/rfc-choices.md`, and add any new one there.
- Every decoder builds records through `RecordBuilder`, so the per-record rules
  are the same in all formats. Keep it that way.
- A record of base fields only (unknown labels don't count) is context, not a
  measurement.
- Unknown labels are ignored or kept as extensions; labels ending in `_` are
  refused.
- Numbers must be finite. Versions above 10 are refused.
- The units registry is RFC 8428's Table 6. Later RFCs (9100, 8798, 9193, 8790)
  are not implemented; the README's Standards table says so. Don't claim them.
- EXI is strict, with the section 8 schema and schemaId `"a"`. It cannot carry
  extensions or -0.0.

Report only what tests show. Don't claim full conformance from passing examples.

## no_std and memory usage

- Don't add `std` dependencies to library crates.
- Every new dependency must build for `thumbv7em-none-eabihf` with `alloc` only.
- Avoid `core::fmt` and `core`'s float parser in library paths: errors use fixed
  strings, and decimal text goes through `rsenml_core::codec::number`.
- Keep fixed-buffer output (`encode_into`) working in every format.

## Error handling

All crates return `rsenml_core::Error`. Use its existing variants; don't add a
variant without checking the existing ones, and keep the record index in
per-record errors. Don't:

- accept invalid SenML because another library does;
- weaken validation to make an interoperability test pass;
- write a different representation without telling the caller (EXI extensions
  are dropped only because the RFC says so, and `Extensions::Error` refuses them).

## Security

Treat every input as hostile. Keep:

- the JSON nesting limit (`MAX_DEPTH`) and skipping of nested extension values;
- the handling of huge announced CBOR lengths (tested in `tests/robustness.rs`);
- the XML refusal of DTDs, and UTF-8 only;
- `tests/robustness.rs` (cut-off and mutated input, hostile cases) and the fuzz
  targets in `fuzz/`.

Changes to any decoder need robustness tests.

## Interoperability

- EXI output must equal EXIficient 1.0.7's bytes on every case
  (`crates/rsenml-tests/fixtures/exificient`). Never regenerate those fixtures to
  make a failing test pass.
- RFC 8428's own CBOR and EXI dumps must come out byte for byte.
- SenMLFormat (Julia) tests against `crates/rsenml-tests/fixtures/export`. Change
  behaviour here first, re-export with `scripts/export-fixtures.sh`, then port it.

## Development conventions

1. Read the crate and its public API before changing it.
2. Check the user guide and examples.
3. Keep the crate boundaries: data model in core, one format per crate.
4. Add tests for what is accepted and what is refused.

Don't invent API methods, feature flags or error variants. Check the source first.

Write comments in plain, everyday words, and link the RFC section
(`https://www.rfc-editor.org/rfc/rfc8428#section-...`) where code follows it.

## Testing

Read `CONTRIBUTING.md` for the full procedure. Run `scripts/gate.sh` before every
commit; `--quick` skips the EXIficient check (Java) and the RFC excerpt check (the
downloaded RFC).

## Documentation

- Keep the README about what rsenml does and how to use it. Development steps go in
  `CONTRIBUTING.md`.
- Update the guide (`guide/`) when a public API or behaviour changes, and keep
  `examples/basics` building, with each chapter's output pasted from a real run.
- `guide/src/comparison.md` comes only from `compare/`.
- Prefer direct technical explanations over marketing language.

## Repository rules

- Never commit `CR.md` files.
- No third-party code in the tree; pin it in `third-party.lock`.
- Of the standards, commit only cited excerpts in
  `crates/rsenml-tests/fixtures/rfc8428`, never a whole RFC.
