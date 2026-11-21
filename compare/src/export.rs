//! `cargo run --release -- export <dir>`: writes the conformance vectors and the
//! test cases as plain TSV files, so other implementations (SenMLFormat.jl) can check
//! that they give the same results and the same bytes as rsenml.
//!
//! `vectors.tsv`: one line per conformance vector (`vectors::all`):
//!   id, section, basis, fmt, expect, rsenml, input, expected, json, cbor, xml,
//!   exi_bp, exi_ba, desc
//! - `fmt` is `json`, `cbor` or `cbor-sensml` (an indefinite-length array);
//! - `expect` is `accept` or `reject` (what RFC 8428 asks for);
//! - `rsenml` is `ok`, or the name of rsenml's `Error` variant;
//! - `input` is hex; `expected` is the oracle's records as JSON text, or `-`;
//! - the five encodings are rsenml's bytes (hex) for the pack it decoded, `-`
//!   when it did not decode, or `!` and the `Error` variant when encoding failed.
//!
//! `cases.tsv`: one line per test case (`rsenml_tests::all_cases`):
//!   name, json, cbor, cbor_sensml, xml, exi_bp, exi_ba  (all hex)
//!
//! Cells never hold a tab or a newline. `scripts/export-fixtures.sh` writes the
//! files into `crates/rsenml-tests/fixtures/export` and checks them.

use std::fmt::Write as _;
use std::path::Path;

use rsenml::exi::{Alignment, EncodeOptions};
use rsenml::{Error, Pack};

use crate::libs::Fmt;
use crate::model::to_json;
use crate::vectors::{self, Basis, Expect};

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// The variant name of an error (`DuplicateLabel`), from its `Debug` text.
fn kind(e: &Error) -> String {
    let d = format!("{e:?}");
    d.split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn cell(r: rsenml::Result<Vec<u8>>) -> String {
    match r {
        Ok(b) => hex(&b),
        Err(e) => format!("!{}", kind(&e)),
    }
}

fn exi(pack: &Pack<'_>, alignment: Alignment) -> String {
    cell(rsenml::exi::encode_with(
        pack,
        &EncodeOptions::new().with_alignment(alignment),
    ))
}

/// rsenml's bytes for one pack: JSON, CBOR, XML, EXI bit-packed, EXI byte-aligned.
fn encodings(pack: &Pack<'_>) -> [String; 5] {
    [
        cell(rsenml::json::encode(pack)),
        cell(rsenml::cbor::encode(pack)),
        cell(rsenml::xml::encode(pack)),
        exi(pack, Alignment::BitPacked),
        exi(pack, Alignment::ByteAligned),
    ]
}

pub fn vectors_tsv() -> String {
    let mut out = String::from(
        "id\tsection\tbasis\tfmt\texpect\trsenml\tinput\texpected\tjson\tcbor\txml\texi_bp\texi_ba\tdesc\n",
    );
    for v in vectors::all() {
        let fmt = match v.fmt {
            Fmt::Json => "json",
            Fmt::Cbor => "cbor",
            Fmt::CborSensml => "cbor-sensml",
        };
        let basis = match v.basis {
            Basis::Normative => "normative",
            Basis::Reading => "reading",
        };
        let (expect, expected) = match &v.expect {
            Expect::Accept(records) => ("accept", to_json(records)),
            Expect::Reject => ("reject", "-".to_owned()),
        };
        let decoded = match v.fmt {
            Fmt::Json => rsenml::json::decode(&v.input).map(Pack::into_owned),
            Fmt::Cbor => rsenml::cbor::decode(&v.input).map(Pack::into_owned),
            Fmt::CborSensml => rsenml::cbor::decode_sensml(&v.input).map(Pack::into_owned),
        };
        let (verdict, enc) = match &decoded {
            Ok(p) => ("ok".to_owned(), encodings(p)),
            Err(e) => (kind(e), std::array::from_fn(|_| "-".to_owned())),
        };
        let row = [
            clean(&v.id),
            v.section.to_owned(),
            basis.to_owned(),
            fmt.to_owned(),
            expect.to_owned(),
            verdict,
            hex(&v.input),
            clean(&expected),
        ]
        .into_iter()
        .chain(enc)
        .chain([clean(&v.desc)])
        .collect::<Vec<_>>()
        .join("\t");
        out.push_str(&row);
        out.push('\n');
    }
    out
}

pub fn cases_tsv() -> String {
    let mut out = String::from("name\tjson\tcbor\tcbor_sensml\txml\texi_bp\texi_ba\n");
    for (name, pack) in rsenml_tests::all_cases() {
        let [json, cbor, xml, bp, ba] = encodings(&pack);
        let sensml = cell(rsenml::cbor::encode_sensml(&pack));
        let row = [clean(&name), json, cbor, sensml, xml, bp, ba].join("\t");
        out.push_str(&row);
        out.push('\n');
    }
    out
}

pub fn run(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("vectors.tsv"), vectors_tsv())?;
    std::fs::write(dir.join("cases.tsv"), cases_tsv())
}
