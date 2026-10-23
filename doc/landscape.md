# Landscape: Rust SenML and EXI libraries

Surveyed 2026-10-13 by reading source (crates.io `.crate` files and GitHub clones; nothing
built or run). Download counts from the crates.io API on that date. RFC references are to
RFC 8428 ([`../spec/rfc8428.txt`](../spec/rfc8428.txt)).

## SenML crates

No Rust crate implements SenML XML (§7) or EXI (§8), the units registry (§12.1, Table 6),
SenSML streaming (§4.8), or rejection of must-understand labels ending in `_` (§4.4).

| | coapum-senml 0.1.0 | sindit-senml 0.3.0 | senml-cbor 0.1.0 | senml-deser 0.2.1 |
|---|---|---|---|---|
| Released | 2025-08-19 | 2026-03-16 (since 2023-11) | 2026-07-27 | 2024-02-16 (only release) |
| License | MIT/Apache-2.0 | Apache-2.0 | MIT/Apache-2.0 | OLFL-1.3 (not OSI) |
| Downloads | 822 | 3,942 | 547 | 12,594 |
| JSON | yes, without base fields | yes | serde derive only | no |
| CBOR | text keys: not SenML-CBOR | no | 8 of 15 Table 4 labels | all 15 labels |
| XML / EXI | stub (`src/xml.rs:32` "not yet implemented") / no | no | no | no |
| `no_std` | no | no | yes, no alloc | yes, no alloc (heapless) |
| Resolution (§4.6) | heuristic, wrong | yes | no | yes |
| `_` labels rejected | no (checks names instead) | no | no | rejects every unknown label |
| `vd` | padded standard base64 (wrong) | unpadded base64url | missing | byte string |
| Dependencies (lockfile) | 106 | 54 | serde (optional) | serde_cbor 0.11 (unmaintained, RUSTSEC-2021-0127) |

Notable defects:

- **coapum-senml**: `SenMLRecord` has no `bn`/`bt`/`bu`/`bv`/`bs`/`bver`, so serde silently drops
  them; CBOR goes through the JSON serde derive (text keys, `vd` as text); the canonical
  option does the same in both branches. The "full RFC 8428 compliance with JSON, CBOR and
  XML" claim is false.
- **sindit-senml**: the most correct. Accepts `_` labels (kept as `extra_fields`) and
  `bver` > 10; a record with no value gets `v = 0`; only resolved records are public.
- **senml-cbor**: rejects CBOR additional info 26/27, so integers >= 65536 (an integer Unix
  time) fail to decode; no indefinite-length arrays (SenSML); every float written as 8
  bytes; a 128-byte name limit not in the RFC.
- **senml-deser**: `Label::try_from(key).unwrap()` (`src/content/record.rs:402`) panics on an
  unknown integer label from untrusted input; text-label extensions are hard errors;
  `Pack` holds 32 records and panics past that; its one RFC test passes even when decoding
  fails (`if let Ok`).

## EXI libraries

On crates.io the only EXI crate is `iso15118`; nothing is published as `erxi`, `exi`,
`exi-rs` or `exi4json`.

| | rexi | erxi | iso15118 (hupe1980) | iso15118-encoders-rs | wlcx/exi | exirs |
|---|---|---|---|---|---|---|
| Pure Rust | yes (unsafe only in rexi-capi) | yes, 8 unsafe sites | yes, `forbid(unsafe_code)` | no: bindgen over cbexigen C, links `-liso15118 -lgnutls` | yes | no: exip C |
| `no_std` | yes + alloc (since 2026-10-13) | no | yes + alloc | no | no | no |
| Arbitrary XSD | runtime + static tables | runtime | codegen, V2G-tuned | no | no | via exip |
| Strict | yes | yes | no (non-strict only) | fixed by cbexigen | n/a | yes |
| Non-strict / schema-less | no / no | yes / yes | yes / no | V2G only | - / partial decode | yes / yes |
| Bit-packed / byte-aligned | yes / yes | yes / yes | yes / no | yes / no | decode only | per exip |
| (Pre-)compression | no | yes | no | no | no | per exip |
| Options document, cookie | yes | yes | no | no | no | yes |
| EXI4JSON | no | yes | no | no | no | no |
| API | events, serde, C ABI | events, CLI | generated structs | wrappers over C structs | decoder | events |
| EXIficient evidence | byte-exact fixtures, W3C 342/3172 | cross matrix; expectations allow 848 DIFF rows | goldens, 298 grammars diffed | none | none | none |
| License | BSD-3-Clause | PolyForm Noncommercial 1.0.0 | MIT/Apache-2.0 | Apache-2.0 | MIT | MIT/Apache-2.0 |
| Activity | active | Feb 2026 burst, quiet since | very active (Aug-Sep 2026) | dormant since 2025-03 | 2026-03 | 2024-05 |

Notes:

- **erxi** (github.com/schachmadinejad/erxi, GitHub only, about 65k lines) is the broadest
  feature set. Its docs claim byte identity with EXIficient on declared fixtures, but
  `tests/cross_matrix_expectations.tsv` accepts DIFF for 424 variants per test, 89 of
  them strict, and `scripts/verify.sh` treats DIFF as a pass: parity is unproven. "Lightning
  fast" is unverifiable (the benchmark repo 404s). Non-commercial license: read, never copy.
- **iso15118-encoders-rs** is not "native without external dependency chains": it needs the
  system-installed cbexigen C library, gnutls and bindgen, and only does DIN 70121 / ISO
  15118-2.
- **iso15118** is the closest in spirit to rexi's direction (no_std + alloc, forbid unsafe,
  codegen, EXIficient goldens) but cannot do RFC 8428 §8, which requires strict.

Only rexi and erxi can take the SenML XSD today.

## What rsenml will measure

1. **Conformance checklist** from the RFC: every label in Tables 1, 2, 4, 5; resolution
   (§4.6) incl. `bv`/`bs` addition and `bver`; `_` labels are errors, other unknown labels
   ignored, in every format; name rules (§4.5.1); at most one of `v`/`vs`/`vb`/`vd`;
   relative time below 2^28 (§4.5.3); `vd` as unpadded base64url (JSON, XML) and byte string
   (CBOR); definite arrays for SenML, indefinite for SenSML (§6); namespace
   `urn:ietf:params:xml:ns:senml` (§7); EXI schemaId "a" (§8); units registry (§12.1).
   Each competitor scored against the same list.
2. **Exact bytes**: the RFC's 195-byte CBOR dump (§6) and 49-byte byte-aligned EXI dump (§8);
   EXI also byte-equal to EXIficient 1.0.7 in strict bit-packed, byte-aligned and
   with-options profiles (through exip's `utils/xsd2exip`).
3. **Sizes** on the §5.1.3 example against Table 3 (JSON 573, XML 649, CBOR 254, EXI 161).
4. **Footprint**: `.text` on `thumbv7em-none-eabihf` per codec and feature set (vs senml-cbor,
   senml-deser); non-dev dependency count per feature set.
5. **Speed and allocation**: encode/decode on the §5.1 packs and a large synthetic pack (> 32
   records, integer times >= 2^16); allocations per message.
6. **Robustness**: fuzzing every decoder, no panics on arbitrary input.
7. **EXI cross-decoding**: rsenml/rexi output decoded by erxi, EXIficient and exipd, and back
   (erxi run sandboxed, from its own directory).

## Reusable test vectors

- RFC 8428 itself: §5.1.1-5.1.7 JSON, §6 CBOR hex, §7 XML, §8 XSD and EXI hex. Code
  components are under the Simplified BSD License (RFC boilerplate).
- sindit-senml `tests/senml_specification_examples.rs` (Apache-2.0): resolved outputs as a
  cross-check; adapt with attribution.
- senml-cbor unit tests (MIT/Apache-2.0): half-precision, NaN/Inf, duplicate keys, tags inside
  unknown keys.
- Not reused: senml-deser (OLFL-1.3), erxi (PolyForm Noncommercial).
- Differential fuzzing oracles: sindit-senml (JSON), senml-cbor (CBOR), with the deviations
  above known.
