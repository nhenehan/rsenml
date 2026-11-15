# rsenml and SenMLFormat side by side

This page shows how each rsenml call looks in SenMLFormat, so you can move between
Rust and Julia.

[SenMLFormat](https://github.com/nhenehan/SenMLFormat.jl) is the Julia version of
rsenml. It writes the same bytes in every format and gives the same verdicts on
the same test vectors, which it takes from rsenml's `fixtures/export`.

| Task | rsenml (Rust) | SenMLFormat (Julia) |
|---|---|---|
| Build a record | `Record::new().with_name("room").with_value(21.5)` | `Record(n = "room", v = 21.5)` |
| Build a pack | `[r1, r2].into_iter().collect::<Pack>()` | `Pack([r1, r2])` |
| Write JSON | `rsenml::json::encode(&pack)?` | `SenMLFormat.Json.encode(pack)` |
| Read CBOR | `rsenml::cbor::decode(&bytes)?` | `SenMLFormat.Cbor.decode(bytes)` |
| Byte-aligned EXI | `exi::encode_with(&pack, &EncodeOptions::new().with_alignment(Alignment::ByteAligned))` | `SenMLFormat.Exi.encode(pack; alignment = :byte_aligned)` |
| By media type | `rsenml::encode(MediaType::SenmlCbor, &pack)` | `SenMLFormat.encode(SENML_CBOR, pack)` |
| Check | `pack.validate()` | `validate(pack)` |
| Resolve | `pack.resolve(Some(now))` | `resolve(pack; now)` |
| Errors | `Result` with `rsenml::Error` | `SenMLError`, whose `kind` is named after rsenml's variants |

Only in rsenml: `no_std` and fixed buffers (`encode_into`).
