# Contributing to rsenml

This document covers the crates, the build and test commands, the generated files
and the test data.

## Crates

| Crate | Purpose | Built on |
|---|---|---|
| `rsenml` | Facade: re-exports, one feature per format, `decode`/`encode` by media type | the crates below |
| `rsenml-core` | Labels, records, packs, validation (section 4), resolving (4.6), units registry (12.1), `rec=` fragments (9), media types and CoAP Content-Formats (12.3, 12.5), base64url, number forms, and the `RecordBuilder` every decoder uses | nothing |
| `rsenml-json` | JSON (section 5) | nanojson 0.6 |
| `rsenml-cbor` | CBOR (section 6) | minicbor 2.3 (`alloc`) |
| `rsenml-xml` | XML (section 7) | xmlparser 0.13 |
| `rsenml-exi` | EXI (section 8) | rexi-core 0.3 |
| `rsenml-tests` | RFC vectors, round trips, EXIficient bytes, robustness (not published) | |

Every library crate is `#![no_std]` with `extern crate alloc`, forbids `unsafe`,
and builds for `thumbv7em-none-eabihf`, alone or together.

`examples/`, `compare/` and `fuzz/` are separate workspaces. They are built only
on request, never by the workspace or the gate.

## Gate and commands

`scripts/gate.sh` runs everything a commit must pass. `--quick` skips the two
checks that need more than Rust: the EXIficient bytes (Java) and the RFC excerpts
(the downloaded RFC).

```bash
scripts/fetch-third-party.sh          # check out the pinned rexi (schema generator) and exip (oracle)
scripts/gate.sh                       # everything below, plus the excerpt and EXIficient checks
scripts/gate.sh --quick

cargo fmt --all --check
cargo clippy --workspace --all-targets --no-deps -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
cargo +1.88 test --workspace          # oldest supported Rust; rustup toolchain install 1.88
cargo build -p rsenml --target thumbv7em-none-eabihf [--no-default-features --features cbor]
fuzz/run.sh [seconds]                 # every fuzz target, 60 s each (nightly, cargo install cargo-fuzz)
```

| Script | Purpose |
|---|---|
| `scripts/fetch-third-party.sh [--check \| --update NAME]` | Pinned tools from `third-party.lock` in `~/.cache/rsenml/third_party` |
| `scripts/fetch-standards.sh [--check]` | The full RFC 8428 text, checked by sha256 (`standards.lock`), in `~/.cache/rsenml/standards` |
| `scripts/gen-tables.sh [--check]` | EXI grammar tables of the section 8 schema |
| `scripts/gen-exi-fixtures.sh [--check]` | EXIficient 1.0.7 reference bytes |
| `scripts/export-fixtures.sh [--check]` | Vectors and cases for SenMLFormat |

## RFC 8428 excerpts

The tests read the RFC's examples, hex dumps and schema from
`crates/rsenml-tests/fixtures/rfc8428`. Each file is quoted as printed, and its
README gives the citation, the section of each file and the IETF Trust license.
Only the parts the tests need are committed, never the whole RFC.

`crates/rsenml-tests/tests/extract.rs` keeps them honest: it cuts each excerpt out
of the full RFC text again and compares. It needs `scripts/fetch-standards.sh`,
and skips without it unless `RSENML_REQUIRE_STANDARDS=1` (the full gate and CI's
`exificient` job set it). `RSENML_WRITE_EXCERPTS=1` writes the files again.

## Generated tables

`scripts/gen-tables.sh` writes `crates/rsenml-exi/src/generated/senml.rs` from the
`senml.xsd` excerpt, with rexi-schema-gen from the rexi tag pinned in
`third-party.lock`. Keep that tag at the same version as the `rexi-core`
dependency. The tables use schemaId `"a"`, so rexi refuses a stream that names
another schema. Never edit them by hand; `--check` fails when they are stale.

## Reference fixtures

`scripts/gen-exi-fixtures.sh` writes the EXIficient 1.0.7 bytes of every test case,
bit-packed and byte-aligned, into `crates/rsenml-tests/fixtures/exificient`,
through exip's oracle `utils/xsd2exip` (Java 17, Maven). `manifest.tsv` records
each stream's size and sha256. `tests/exificient.rs` checks that rsenml writes the
same bytes and reads them back.

## Exported vectors (SenMLFormat)

`scripts/export-fixtures.sh` writes `crates/rsenml-tests/fixtures/export`: the
conformance vectors of `compare/` and every test case, with rsenml's verdicts and
bytes. SenMLFormat, the Julia port, tests against these files at a tagged rsenml
version. Change behaviour here first, then re-export, tag, and sync SenMLFormat.

## Third parties

Nothing third-party lives in the working tree. `scripts/fetch-third-party.sh`
checks out the pins of `third-party.lock` into
`${RSENML_THIRD_PARTY:-${XDG_CACHE_HOME:-~/.cache}/rsenml/third_party}`:

- **rexi**, for rexi-schema-gen (`gen-tables.sh`). Cargo gets `rexi-core` itself
  from crates.io.
- **exip**, for its EXIficient oracle (`gen-exi-fixtures.sh`).

## Design notes

### Library choices

The aim was the lightest `no_std` library per text/binary format. Two of the three
first choices did not survive contact with the source:

- **JSON: nanojson instead of serde-json-core.** serde-json-core 0.7 cannot read a
  value whose type is not known in advance (`deserialize_any` is unsupported), so
  extension values could not be kept, and its number reader collects
  `[0-9+-.eE]` and hands the run to `f64::from_str`, accepting `+1`, `.5`, `1.` and
  `null` (as NaN). nanojson is a zero-dependency pull parser that checks the JSON
  grammar (leading zeros, exponent, escapes, surrogate pairs, control characters),
  gives error offsets and raw number text, and has a writer with `number_raw` for
  rsenml's own number formatting. picojson was the other candidate; it exposes no byte
  positions, which record-at-a-time streaming needs.
- **XML: xmlparser instead of quick-xml.** quick-xml requires `std`. xmlparser is a
  `no_std`, zero-dependency tokenizer; namespaces, end-tag matching, references and
  attribute-value normalization are done in `rsenml-xml` (a two-level document makes
  that small). Writing XML needs no library.
- **CBOR: minicbor, as planned**, without its `half` feature: half-precision floats
  (the RFC's own example uses one) are converted in `rsenml_core::codec::number`, which
  saves the `half` crate.
- **EXI: rexi**, through rexi's `no_std` support: `rexi-core` 0.3 from
  [crates.io](https://crates.io/crates/rexi-core). The grammar tables come from
  rexi-schema-gen at the matching tag (`v0.3.0`, in `third-party.lock`). They are
  generated with schemaId `"a"`, so rexi itself refuses a stream that names
  another schema.

### Data model

`Record<'a>` holds the fields as written (base fields included) as `Option`s; text
and data are `Cow`s, so the CBOR and XML readers borrow from the input where they
can. `RecordBuilder` is shared by every decoder, so the per-record rules (one
occurrence per label, at most one value, the label's type, finite numbers, `_`
labels) are the same in all formats. The guide's
[Where the RFC is unclear](https://nhenehan.github.io/rsenml/rfc-choices.html)
has the full data model and the readings of the RFC.

### Footprint

Small `no_std` builds are dominated by `core`'s float parser and by `core::fmt`,
so the library code avoids both where it can:

- Decimal text and decimal fractions are converted by
  `rsenml_core::codec::number::decimal_to_f64` / `parse_decimal`: Clinger's fast path
  (mantissa within 53 bits, |exponent| <= 22) and otherwise a fixed-size big
  integer with one correctly rounded step. Tested against `core`'s parser on
  300 000 random values, halfway cases and the subnormal and overflow limits.
- Error reasons are fixed strings (or concatenations), never `format!` of a
  library's error.

`doc/comparison.md` has the measured sizes of the `compare/` footprint binaries
(`thumbv7em-none-eabihf`, `opt-level = "z"`). Most of what is left in the JSON
build is `core`'s shortest float printing, which writing text needs; a
decode-only build does not link it.

## Documentation

- The user guide is `guide/` (mdBook with mermaid). Its programs are
  `examples/basics`; each chapter includes one with `{{#include}}` and shows its
  real output. Run `examples/basics/run-all.sh` after a change and update the
  "Run it" blocks. `.github/workflows/guide.yml` publishes it to GitHub Pages
  on every push to `main` that changes it.
- `doc/comparison.md` is written by `compare/` (`cargo run --release` there).
  Never edit it by hand.
- `doc/landscape.md` is a dated survey of the other Rust SenML and EXI crates.

## Releasing

1. Bump the version in `[workspace.package]` and in the rsenml entries of
   `[workspace.dependencies]` in `Cargo.toml`, and add a `## X.Y.Z - YYYY-MM-DD`
   section to `CHANGELOG.md`.
2. Push to `main` and wait for CI (`.github/workflows/ci.yml`) to pass.
3. Push the tag `vX.Y.Z`. The release workflow (`.github/workflows/release.yml`)
   runs CI again, checks that the tag matches the version and that the crates
   package, and then waits for an approval of the `crates-io` environment on the
   run's page. After the approval it publishes every crate to crates.io and makes
   the GitHub Release from the CHANGELOG section.
4. If crates.io stops the publish partway (for example its limit on new crates),
   use **Re-run failed jobs**: crates already on crates.io at that version are
   skipped.

Set up once, before the first tag:

- the `crates-io` environment: you as the required reviewer, tags matching `v*`
  only, and the `CARGO_REGISTRY_TOKEN` secret in the environment (not the
  repository);
- GitHub Pages: Settings > Pages > Source: **GitHub Actions**, for the guide.
