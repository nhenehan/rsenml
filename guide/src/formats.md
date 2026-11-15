# Choosing a format

In this chapter you'll write the same pack in every SenML media type and compare
their sizes.

```rust,ignore
{{#include ../../examples/basics/src/bin/formats.rs:main}}
```

Run it with `cargo run --bin formats`:

```text
application/senml+json       CoAP 110: 143 bytes
application/sensml+json      CoAP 111: 143 bytes
application/senml+cbor       CoAP 112:  91 bytes
application/sensml+cbor      CoAP 113:  92 bytes
application/senml+xml        CoAP 310: 202 bytes
application/sensml+xml       CoAP 311: 202 bytes
application/senml-exi        CoAP 114:  79 bytes
application/sensml-exi       CoAP 115:  79 bytes
EXI byte-aligned: 98 bytes
Some(SenmlCbor)
```

`rsenml::encode` and `rsenml::decode` take a `MediaType`, so a server can pick
the format from a request's `Content-Type` header or CoAP Content-Format option.
`MediaType::from_content_type` and `MediaType::from_coap_content_format` turn
those into a `MediaType`.

## Which one to use

| Format | Good for |
|---|---|
| JSON | Web APIs and people reading logs. Every language can read it. |
| CBOR | Small devices and CoAP: compact, and simple to write. |
| XML | Systems that already speak XML. |
| EXI | The smallest messages, for networks where every byte counts. |

The SenSML media types (`sensml`) mark the data as a stream. The bytes are the
same as SenML, except in CBOR, where SenSML uses an array with no length up front
(one byte more here), so the writer doesn't need to know how many records are
coming.

## EXI options

SenML EXI is strict, schema-informed EXI with the schema of
[RFC 8428 section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8). rsenml
writes the header with its options (schema ID `"a"`), as the RFC asks, so any
EXI reader with that schema can read it.

- **Bit-packed** (the default) is the smallest.
- **Byte-aligned** puts every value on a byte boundary. It is a little bigger,
  but a simple sensor can fill in a byte-aligned template by hand, as the RFC's
  second section 8 example shows.

`EncodeOptions` also turns on the `$EXI` cookie (the RFC advises against it over CoAP and HTTP) and chooses what happens to
extensions: strict EXI has no place for them, so they are dropped by default, or
refused with `Extensions::Error`. `DecodeOptions::with_out_of_band` reads
streams whose header leaves the options out.
