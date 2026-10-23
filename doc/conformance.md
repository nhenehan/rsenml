# RFC 8428 conformance checklist

Each requirement, where rsenml implements it, and the test that shows it.
Test paths are relative to `crates/`. "All formats" means JSON, CBOR and XML read
the same way through `RecordBuilder` (EXI's strict grammar cannot express most
violations in the first place).

| # | Requirement | RFC | Implementation | Tests |
|---|---|---|---|---|
| 1 | All labels of Tables 1, 2, 4, 5 with their types | 4.3, 5, 6, 7 | `rsenml-core/src/label.rs` | `label.rs` tests; `rsenml-tests/tests/cross_format.rs` (all-fields case) |
| 2 | CBOR integer keys exactly as Table 4 | 6 | `Label::cbor` | `rsenml-cbor/tests/cbor.rs` `rfc8428_section_6_both_ways` |
| 3 | Unknown labels ignored (kept as extensions) | 4.4 | `RecordBuilder::extension` | `json.rs` `unknown_labels_are_kept_or_skipped`, `cbor.rs` `unknown_integer_keys_are_ignored...` |
| 3a | CBOR text key spelling a registered label (`"v"`) is an unrecognized field: ignored, value dropped | 4.4, 6, 12.2; RFC 9193 8 | `rsenml-cbor/src/read.rs` `entry` | `cbor.rs` `text_keys_spelling_registered_labels_are_ignored` |
| 4 | Unknown label ending in `_` is an error | 4.4 | `RecordBuilder` | `json.rs`, `cbor.rs`, `xml.rs` `record_rules` / `xsd_types` |
| 5 | Version: positive integer, same in every record, newer than 10 refused; older (the RFC's `bver: 5`) accepted, although RFC 9100's bitmap reading would not | 4.1, 4.4; RFC 9100 2-3 | `Validator::check_version` | `validate.rs` `versions`; `json.rs` `record_rules` |
| 6 | Exactly one value unless a sum is present | 4.2 | `RecordBuilder::set_value`, `Validator` | `builder.rs`, `validate.rs` `value_and_name_rules` |
| 7 | Name optional when a base name is present | 4.2 | `Validator` | `validate.rs` `value_and_name_rules` |
| 8 | Concatenated name character set and first character | 4.5.1 | `valid_name` | `validate.rs` `value_and_name_rules` |
| 9 | Unit falls back to base unit | 4.5.2 | `Resolver` | `resolve.rs` `rfc8428_section_5_1_4` |
| 10 | Time = bt + t; below 2^28 relative to now, else absolute | 4.5.3 | `Resolver`, `ABSOLUTE_TIME_THRESHOLD` | `rsenml-tests/tests/rfc_examples.rs` `section_5_1_7_actuation_times` |
| 11 | Sum = bs + s when either present; value = bv + v | 4.5.4 | `Resolver` | `resolve.rs` `base_value_sum_relative_time_and_order` |
| 12 | Resolved records: no base fields, no relative times, chronological, `bver` only when not 10 | 4.6 | `Pack::resolve`, `ResolvedRecord::into_record` | `rfc_examples.rs` `section_5_1_4_is_section_5_1_3_resolved` |
| 13 | Base values apply until the next record with the same base field | 4 | `Resolver` | `rfc_examples.rs` `section_5_1_6_mixes_base_names` |
| 14 | SenSML: records usable before the stream ends | 4.8 | `json::StreamDecoder`, `cbor::StreamDecoder` | `rfc_examples.rs` `the_sensml_example_streams_record_by_record`; `json.rs`, `cbor.rs` `stream_decoder_takes_bytes_as_they_come` |
| 15 | JSON: UTF-8, RFC 8259 escapes, IEEE double range, lower-case `e` | 5 | nanojson; `number::write_number` | `json.rs` `escapes_in_strings_and_keys`, `json_grammar_is_enforced`, `numbers_keep_their_value...` |
| 16 | Data Value: unpadded base64url in JSON and XML, byte string in CBOR | 4.3, 5, 6, 7 | `base64url`, CBOR `bytes` | `base64url.rs`; `json.rs`/`xml.rs` `record_rules`; `cbor.rs` |
| 17 | CBOR numbers convert back to the same double; `bver` unsigned only | 6 | `write_number`, `numeric` | `cbor.rs` `numbers_take_the_smallest_exact_form`, `every_numeric_form_is_read`, `record_rules` |
| 18 | CBOR: definite-length strings; definite array for SenML, indefinite for SenSML | 6 | `rsenml-cbor` | `cbor.rs` `sensml_uses_an_indefinite_array`, `record_rules` |
| 19 | XML: UTF-8, namespace `urn:ietf:params:xml:ns:senml`, `sensml`/`senml` elements, Table 5 attributes | 7 | `rsenml-xml` | `xml.rs` `rfc8428_section_7`, `namespaces_prefixes_and_foreign_attributes`, `structure_is_enforced` |
| 20 | EXI: strict schema mode with the RFC's XSD | 8 | `rsenml-exi`, `spec/senml.xsd`, `scripts/gen-tables.sh` | `rsenml-tests/tests/extract.rs` (XSD equals the RFC's) |
| 21 | EXI header includes options with schemaId "a"; no cookie by default | 8 | `rsenml-exi` | `exi.rs` `schema_id_is_checked`, `cookie_and_fixed_buffer` |
| 22 | EXI bit-packed and byte-aligned | 8 | `Alignment` | `exi.rs` `rfc8428_section_8_*_both_ways` |
| 23 | Units registry | 12.1 | `units` (Table 6), `ValidationOptions::registered_units` | `units.rs`, `validate.rs` `extensions_and_units` |
| 24 | `rec=` fragment identifiers | 9 | `fragment` | `fragment.rs` `rfc8428_section_9_1` |
| 25 | Media types and CoAP Content-Formats | 12.3, 12.5 | `MediaType` | `media.rs` `round_trips` |

## The RFC's examples

All extracted from `spec/rfc8428.txt` at test time (`rsenml-tests/src/lib.rs`):

| Vector | Result |
|---|---|
| JSON examples 5.1.1 - 5.1.7 (11 arrays) | all valid; 5.1.2's SenSML stream yields its 9 records before the stream closes |
| 5.1.4 | equals 5.1.3 resolved, and written back gives 5.1.4 exactly |
| CBOR hex dump, section 6 (195 bytes) | decoded = 5.1.2 (with its explicit `t: 0`); encoded byte for byte |
| XML, section 7 | decoded = 5.1.2 |
| EXI hex dump, section 8, bit-packed (63 bytes) | decoded = its XML = 5.1.2's first example; encoded byte for byte |
| EXI hex dump, section 8, byte-aligned (49 bytes) | decoded = its XML; encoded byte for byte; the hand-coded sensor edit works |
| Table 3, EXI size of 5.1.3 | 161 bytes, as in the RFC (JSON 401, XML 510 and CBOR 245 are smaller than the RFC's 573, 649, 254: no whitespace, smallest floats) |
| JSON of 5.1.2's first example, whitespace removed | re-encoded byte for byte (`rsenml-json/tests/json.rs`) |

## Beyond the RFC's examples

- EXI: 16 cases (the RFC examples and synthetic packs with every field, extreme
  doubles, Unicode, 200 records, base-only records), bit-packed and byte-aligned:
  all 32 streams byte-identical to EXIficient 1.0.7 (`rsenml-tests/tests/exificient.rs`).
- Every case round-trips through all eight media types and both EXI alignments,
  and JSON -> CBOR -> XML -> EXI -> JSON gives the original JSON
  (`rsenml-tests/tests/cross_format.rs`).
- No decoder panics on any truncation or on 64 random mutations (2000 in release
  runs) of every case in every format, fed to every decoder, nor on deep nesting,
  oversized CBOR lengths or an XML entity bomb (`rsenml-tests/tests/robustness.rs`).
