//! Section 1: features. One table of what each library's API offers, and one
//! of RFC 8428 features, where each cell sums up the conformance vectors that
//! test that feature (section 2 has the vectors one by one).

use std::fmt::Write as _;

use crate::conformance::{Outcome, Row};
use crate::env::DepRow;
use crate::libs::Lib;

const LIBS: [Lib; 4] = [Lib::Rsenml, Lib::Sindit, Lib::Coapum, Lib::SenmlCbor];

/// (feature, RFC sections, vector ids). Every vector must be in exactly one
/// row, so a new vector cannot be left out of this table by mistake.
const MEASURED: &[(&str, &str, &[&str])] = &[
    (
        "RFC JSON examples",
        "5.1",
        &[
            "rfc-5.1.1",
            "rfc-5.1.2a",
            "rfc-5.1.2b",
            "rfc-5.1.3",
            "rfc-5.1.4",
            "rfc-5.1.5",
            "rfc-5.1.6",
            "rfc-5.1.7a",
            "rfc-5.1.7b",
            "rfc-5.1.7c",
        ],
    ),
    ("RFC CBOR example", "6", &["rfc-6"]),
    (
        "Base fields (`bn`, `bt`, `bu`, `bv`, `bs`)",
        "4.1, 4.5, 6",
        &["j-base-sum-value", "c-all-labels"],
    ),
    (
        "Version (`bver`) checks",
        "4.1, 4.4",
        &[
            "j-bver-11",
            "j-bver-mixed",
            "j-bver-float",
            "c-bver-float",
            "c-bver-11",
        ],
    ),
    (
        "Unknown labels ignored, `_` labels rejected",
        "4.4",
        &[
            "j-must-understand",
            "j-unknown-label",
            "c-unknown-int-key",
            "c-unknown-int-key-map",
            "c-text-v-extra",
            "c-must-understand",
        ],
    ),
    (
        "One value or a sum per record",
        "4.2",
        &[
            "j-two-values",
            "j-no-value",
            "j-sum-only",
            "c-two-values",
            "c-no-value",
            "c-text-v-only",
        ],
    ),
    (
        "Name rules",
        "4.5.1",
        &[
            "j-name-space",
            "j-name-start",
            "j-name-nonascii",
            "j-name-missing",
            "c-name-invalid",
        ],
    ),
    (
        "Value types (`vd` as unpadded base64url or bytes, escapes)",
        "5, 6",
        &[
            "j-vd",
            "j-vd-padded",
            "j-vd-standard",
            "j-v-string",
            "j-vb-number",
            "j-escapes",
            "c-vd-text",
            "c-vs-bytes",
        ],
    ),
    (
        "Relative, integer and update times",
        "4.2, 4.5.3, 6",
        &["j-relative-time", "j-ut", "c-int-time"],
    ),
    (
        "CBOR half and single floats, decimal fractions",
        "6",
        &["c-half", "c-single", "c-tag4"],
    ),
    ("More than 32 records", "6", &["c-40-records"]),
    (
        "SenSML streams (closed JSON stream; indefinite CBOR array)",
        "4.8, 6",
        &["rfc-5.1.2c", "c-indefinite-sensml"],
    ),
    (
        "Malformed input rejected",
        "5, 6, 11",
        &[
            "j-dup-key",
            "j-empty",
            "j-not-array",
            "j-trailing",
            "j-nan",
            "j-1e400",
            "c-dup-key",
            "c-empty",
            "c-trailing",
            "c-nan",
            "c-infinity",
            "c-indefinite-senml",
        ],
    ),
];

/// "yes", "no", "k of n", or "n/a" for one library over a set of vectors.
fn verdict(rows: &[Row], ids: &[&str], lib: Lib) -> String {
    let (mut pass, mut total, mut panics) = (0, 0, 0);
    for row in rows.iter().filter(|r| ids.contains(&r.vector.id.as_str())) {
        for (l, o) in &row.outcomes {
            if *l != lib {
                continue;
            }
            match o {
                Outcome::Pass(_) => {
                    pass += 1;
                    total += 1;
                }
                Outcome::Fail(_) => total += 1,
                Outcome::Panic(_) => {
                    panics += 1;
                    total += 1;
                }
                Outcome::NotApplicable => {}
            }
        }
    }
    let mut s = match (pass, total) {
        (_, 0) => return "n/a".into(),
        (p, t) if p == t => "yes".into(),
        (0, _) => "**no**".into(),
        (p, t) => format!("**{p} of {t}**"),
    };
    if panics > 0 {
        let _ = write!(s, ", {panics} **panic**");
    }
    s
}

/// Checks that every vector is in exactly one row of [`MEASURED`].
fn check_coverage(rows: &[Row]) {
    for row in rows {
        let n = MEASURED
            .iter()
            .filter(|(_, _, ids)| ids.contains(&row.vector.id.as_str()))
            .count();
        assert_eq!(n, 1, "vector {} is in {n} feature rows", row.vector.id);
    }
    for (_, _, ids) in MEASURED {
        for id in *ids {
            assert!(
                rows.iter().any(|r| r.vector.id == *id),
                "feature row names unknown vector {id}"
            );
        }
    }
}

pub fn section(rows: &[Row], deps: &[DepRow]) -> String {
    check_coverage(rows);
    let mut md = String::new();
    let w = &mut md;
    let head = format!(
        "| | {} |\n|---|{}",
        LIBS.map(Lib::name).join(" | "),
        "---|".repeat(LIBS.len())
    );

    let _ = writeln!(w, "## 1. Features\n");
    let _ = writeln!(
        w,
        "### What each library offers\n\nFrom each library's public API, except `no_std`, \
         which is read from the crate source as in section 5.\n"
    );
    let _ = writeln!(w, "{head}");
    let no_std = |lib: Lib| {
        deps.iter()
            .find(|d| d.label.starts_with(lib.name()))
            .map_or("-".to_owned(), |d| d.no_std.clone())
    };
    let api: [(&str, [&str; 4]); 7] = [
        (
            "JSON (RFC §5)",
            ["read, write", "read, write", "read, write", "no"],
        ),
        (
            "CBOR (RFC §6)",
            [
                "read, write",
                "no",
                "read, write (text keys)",
                "read, write",
            ],
        ),
        ("XML (RFC §7)", ["read, write", "no", "no (stub)", "no"]),
        ("EXI (RFC §8)", ["read, write", "no", "no", "no"]),
        (
            "SenSML stream reader (RFC §4.8)",
            ["JSON, CBOR", "no", "no", "no"],
        ),
        (
            "Records as written",
            [
                "yes",
                "no (resolved only)",
                "yes (no base fields)",
                "yes (8 of 15 labels)",
            ],
        ),
        (
            "Resolved records (RFC §4.6)",
            [
                "yes (`Pack::resolve`)",
                "yes",
                "`normalize()`, not tested here",
                "no",
            ],
        ),
    ];
    for (feature, cells) in api {
        let _ = writeln!(w, "| {feature} | {} |", cells.join(" | "));
    }
    let _ = writeln!(w, "| `no_std` | {} |\n", LIBS.map(no_std).join(" | "));

    let _ = writeln!(
        w,
        "### RFC 8428 features, measured\n\nEach cell sums up the section 2 vectors for that \
         feature: *yes* when the library passes all of them, *k of n* when it passes some, \
         *n/a* when it has no reader for those inputs. A pass on a reject vector can be \
         incidental (see the note under section 2's summary).\n"
    );
    let _ = writeln!(
        w,
        "| feature | § | {} |\n|---|---|{}",
        LIBS.map(Lib::name).join(" | "),
        "---|".repeat(LIBS.len())
    );
    for (feature, section, ids) in MEASURED {
        let _ = writeln!(
            w,
            "| {feature} | {section} | {} |",
            LIBS.map(|l| verdict(rows, ids, l)).join(" | ")
        );
    }
    let _ = writeln!(w);
    md
}
