# Where the RFC is unclear

RFC 8428 leaves room in a few places, and in one or two it contradicts itself.
This page lists what rsenml does in each case, so you know what to expect when you
talk to another implementation. [SenMLFormat](https://github.com/nhenehan/SenMLFormat.jl),
the Julia version, makes the same choices.

## Reading the RFC

1. **Records of base fields only.** Section 4.2 requires a value or a sum in every
   record, but the first record of section 5.1.7's example is `{"bn":...}` alone. A
   record with no regular field at all is treated as context for the records after
   it: it needs no value or name and resolves to nothing. Every other record needs
   a value or a sum. An unknown label is ignored (section 4.4), so it does not make a
   base-only record need a value.
2. **Version.** A record's version is the last `bver` at or before it (default 10).
   All records must agree (section 4.4), so a `bver` that differs from the version in
   force is an error, including a later `bver` after records that used the default.
   Versions above 10 are refused ("MUST NOT use"); versions below 10 are accepted
   (the RFC's examples use 5); 0 is not a positive integer. RFC 9100 later made
   `bver` a feature bitmap in which version 10 has bits 0 and 2 clear, so 5 would
   not be a version-10 value there; rsenml keeps accepting it, as RFC 8428's own
   example requires.
3. **Names** (section 4.5.1) are checked on base name + name of measurement records,
   which must be non-empty and match `[A-Za-z0-9][A-Za-z0-9:./_-]*`.
4. **Resolution** (section 4.6): `bv` is added to numeric values only; a record
   without a value field gets none even when `bv` is in force. The sum is `bs + s`
   when either is present (section 4.5.4 literally). Relative times need the
   caller's "now"; resolving one without it is an error. Unknown base extensions
   cannot be resolved and are dropped; regular extensions are kept. Resolved
   records are sorted by time (stable).
5. **Units** are not required to be registered (RFC 8428 does not require it);
   `ValidationOptions::registered_units` makes it a rule. The registry is Table 6
   of RFC 8428 only, without RFC 8798's later additions.
6. **Numbers** must be finite in every representation: JSON cannot carry the
   others, so SenML has no use for them (`xs:double`'s `INF` and `NaN`, CBOR's
   NaN and infinities are refused).
7. **Data Value** text is strict unpadded base64url: padding, the standard
   alphabet and non-zero trailing bits are refused (RFC 4648 section 3.5 allows it).
8. **CBOR keys.** A text key that spells a registered label (`"v"`) is not that
   label in CBOR, where registered labels are Table 4's integers and later labels
   are text keys (RFC 8428 sections 6 and 12.2; RFC 9193 section 8). It is an
   unrecognized field and is ignored (section 4.4 "MUST ignore"); its value is
   skipped, not kept, so a CBOR to JSON translation cannot repeat the label.
   Unknown integer keys are ignored. Text and byte
   strings must have definite lengths (section 6 says so for `vs` and `vd`; rsenml
   applies it to every string). Numbers may be integers, half/single/double floats
   or decimal fractions (tag 4); other tags are refused. `bver` must be an unsigned
   integer. SenML packs must be definite-length arrays; SenSML streams may be
   indefinite (and are written so).
9. **XML.** UTF-8 only (section 7); a DTD is refused (no entity expansion);
   whitespace, comments and processing instructions are allowed between records and
   inside `senml`; attributes in other namespaces are ignored. `bver` is `xs:int`.
10. **EXI.** The header must carry options with schemaId "a" (section 8); streams
    without options are read only when the caller supplies them
    (`DecodeOptions::out_of_band`). Strict mode cannot carry extensions: they are
    dropped by default (section 8 says they will be lost) or refused on request.
    **EXI cannot carry -0.0**: EXI codes a negative integer as its magnitude minus
    one (EXI 1.0 section 7.1.5), so a float mantissa has no negative zero; -0.0 is
    written as 0. JSON, CBOR and XML keep it.

## The data model

`Record<'a>` holds the fields as written (base fields included) as `Option`s;
text and data are `Cow`s, so the CBOR and XML readers borrow from the input where
the representation allows (JSON strings are always copied: nanojson decodes into a
scratch buffer). A record carries at most one value (`Option<Value>`), so a pack
with two value fields cannot be represented and is refused while reading.

Unknown labels (section 4.4) become `Extension`s when their value is a scalar (what
the CDDL of section 11 allows): JSON and CBOR keep their type, XML only has text.
Values that are arrays or maps are skipped. Labels ending in `_` are refused
everywhere, including when their value is skipped.

`RecordBuilder` is shared by every decoder, so the per-record rules (one occurrence
per label, at most one value, the label's type, finite numbers, `_` labels) are
identical in all representations.

## Writing

All writers put fields in the order of the RFC's examples (bn bt bu bv bs bver n u
t ut s v vs vb vd), then extensions, without whitespace. With that order:

- the JSON of section 5.1.2's first example and the CBOR of section 6 come out byte
  for byte;
- numbers use the shortest text that reads back as the same double (`23.1`, `20`,
  `1276020076.001`, `1e300`), with a lower-case `e` (section 5);
- CBOR numbers take the smallest exact form: integer when integral, else half,
  single or double float (section 6 asks that the value convert back exactly);
- EXI doubles are handed to rexi as EXI Floats built from those shortest digits
  with trailing zeros moved into the exponent (`number::decimal_parts`), which is
  how rexi and EXIficient canonicalise the same attribute text: rsenml-exi's bytes
  equal EXIficient's for every test case (`crates/rsenml-tests/fixtures/exificient`),
  without formatting and re-parsing each number.

Encoders validate first (`encode`); `encode_unchecked` (every representation, and
`rsenml::encode_unchecked` by media type) writes a pack as it is, for packs already
validated: validation is about 10-15 % of an EXI encode.
Fixed-buffer variants (`encode_into`) exist for JSON, CBOR and EXI.

## Not implemented

- Partial-input streaming for XML and EXI (see above).
- Non-strict EXI (section 8 allows it to keep extensions); rexi does not implement
  non-strict grammars.
- XPointer fragment identifiers for XML/EXI (section 9.2); `rec=` is implemented.
- RFC 8798's additional units and secondary units.
- RFC 9100's reading of `bver` as a feature bitmap, and the fields RFC 9193 adds
  (see the README's Standards table).
