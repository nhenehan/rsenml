# Changelog

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
