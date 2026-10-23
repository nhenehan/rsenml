//! The conformance vectors: every JSON example of RFC 8428 section 5.1 and the
//! CBOR dump of section 6 (extracted from the RFC text), plus negative and edge
//! vectors. Each has an expected outcome per RFC 8428; for accepted vectors the
//! expected records are JSON text read by the oracle (`model::expected`).

use crate::libs::Fmt;
use crate::model::{expected, NRaw};
use crate::rfc;

/// How firmly the RFC fixes the expected outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// A MUST, a table, or the CDDL of section 11 decides it.
    Normative,
    /// The RFC is silent or indirect; the expectation is this harness's
    /// reading, stated in the description.
    Reading,
}

#[derive(Debug, Clone)]
pub enum Expect {
    Accept(Vec<NRaw>),
    Reject,
}

#[derive(Debug, Clone)]
pub struct Vector {
    pub id: String,
    pub desc: String,
    pub section: &'static str,
    pub basis: Basis,
    pub fmt: Fmt,
    pub input: Vec<u8>,
    pub expect: Expect,
}

fn v(
    id: &str,
    section: &'static str,
    basis: Basis,
    fmt: Fmt,
    desc: &str,
    input: Vec<u8>,
    expect: Expect,
) -> Vector {
    Vector {
        id: id.into(),
        desc: desc.into(),
        section,
        basis,
        fmt,
        input,
        expect,
    }
}

fn accept(json: &str) -> Expect {
    Expect::Accept(expected(json))
}

// ---------------------------------------------------------------- CBOR builder

fn head(major: u8, n: u64) -> Vec<u8> {
    let m = major << 5;
    match n {
        0..=23 => vec![m | n as u8],
        24..=0xff => vec![m | 24, n as u8],
        0x100..=0xffff => {
            let mut v = vec![m | 25];
            v.extend((n as u16).to_be_bytes());
            v
        }
        0x1_0000..=0xffff_ffff => {
            let mut v = vec![m | 26];
            v.extend((n as u32).to_be_bytes());
            v
        }
        _ => {
            let mut v = vec![m | 27];
            v.extend(n.to_be_bytes());
            v
        }
    }
}

/// A CBOR integer (shortest form).
fn int(i: i64) -> Vec<u8> {
    if i >= 0 {
        head(0, i as u64)
    } else {
        head(1, (-1 - i) as u64)
    }
}

fn text(s: &str) -> Vec<u8> {
    let mut v = head(3, s.len() as u64);
    v.extend(s.as_bytes());
    v
}

fn bytes(b: &[u8]) -> Vec<u8> {
    let mut v = head(2, b.len() as u64);
    v.extend(b);
    v
}

fn f64_(x: f64) -> Vec<u8> {
    let mut v = vec![0xfb];
    v.extend(x.to_bits().to_be_bytes());
    v
}

fn f32_(x: f32) -> Vec<u8> {
    let mut v = vec![0xfa];
    v.extend(x.to_bits().to_be_bytes());
    v
}

fn f16_(bits: u16) -> Vec<u8> {
    let mut v = vec![0xf9];
    v.extend(bits.to_be_bytes());
    v
}

fn tag(t: u64, inner: Vec<u8>) -> Vec<u8> {
    let mut v = head(6, t);
    v.extend(inner);
    v
}

fn array(items: Vec<Vec<u8>>) -> Vec<u8> {
    let mut v = head(4, items.len() as u64);
    items.into_iter().for_each(|i| v.extend(i));
    v
}

fn indefinite_array(items: Vec<Vec<u8>>) -> Vec<u8> {
    let mut v = vec![0x9f];
    items.into_iter().for_each(|i| v.extend(i));
    v.push(0xff);
    v
}

fn map(pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
    let mut v = head(5, pairs.len() as u64);
    for (k, val) in pairs {
        v.extend(k);
        v.extend(val);
    }
    v
}

/// One-record pack `[{0: name, ...rest}]`.
fn one(name: &str, rest: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
    let mut pairs = vec![(int(0), text(name))];
    pairs.extend(rest);
    array(vec![map(pairs)])
}

// ---------------------------------------------------------------- vectors

#[allow(clippy::type_complexity)]
pub fn all() -> Vec<Vector> {
    let mut out = Vec::new();
    use Basis::{Normative as N, Reading as R};
    use Fmt::{Cbor as C, CborSensml as CS, Json as J};

    // Every JSON example of section 5.1; the RFC text is its own oracle.
    for ex in rfc::json_examples() {
        let desc = if ex.partial_stream {
            "RFC example (SenSML stream, closed after its last complete record)".to_owned()
        } else {
            "RFC example".to_owned()
        };
        let section = Box::leak(
            ex.name
                .trim_end_matches(char::is_alphabetic)
                .to_owned()
                .into_boxed_str(),
        );
        out.push(v(
            &format!("rfc-{}", ex.name),
            section,
            N,
            J,
            &desc,
            ex.json.clone().into_bytes(),
            accept(&ex.json),
        ));
    }

    let j = |s: &str| s.as_bytes().to_vec();
    let json_cases: Vec<(&str, &'static str, Basis, &str, &str, Expect)> = vec![
        (
            "j-must-understand",
            "4.4",
            N,
            "label ending in `_` must be an error",
            r#"[{"n":"x","v":1,"foo_":1}]"#,
            Expect::Reject,
        ),
        (
            "j-unknown-label",
            "4.4",
            N,
            "unknown label without `_` is ignored",
            r#"[{"n":"x","v":1,"foo":"bar"}]"#,
            accept(r#"[{"n":"x","v":1}]"#),
        ),
        (
            "j-bver-11",
            "4.4",
            N,
            "bver 11 > 10: MUST NOT use",
            r#"[{"bver":11,"n":"x","v":1}]"#,
            Expect::Reject,
        ),
        (
            "j-bver-mixed",
            "4.4",
            N,
            "records with different versions",
            r#"[{"bver":10,"n":"a","v":1},{"bver":9,"n":"b","v":2}]"#,
            Expect::Reject,
        ),
        (
            "j-bver-float",
            "4.1",
            N,
            "bver 10.5: must be a positive integer",
            r#"[{"bver":10.5,"n":"x","v":1}]"#,
            Expect::Reject,
        ),
        (
            "j-two-values",
            "4.2",
            N,
            "both `v` and `vs`: exactly one value field",
            r#"[{"n":"x","v":1,"vs":"a"}]"#,
            Expect::Reject,
        ),
        (
            "j-no-value",
            "4.2",
            N,
            "neither value nor sum",
            r#"[{"n":"x","u":"Cel"}]"#,
            Expect::Reject,
        ),
        (
            "j-sum-only",
            "4.2",
            N,
            "sum without value is allowed",
            r#"[{"n":"x","u":"Ah","s":5}]"#,
            accept(r#"[{"n":"x","u":"Ah","s":5}]"#),
        ),
        (
            "j-base-sum-value",
            "4.5.4",
            N,
            "bv and bs are added to v and s",
            r#"[{"bn":"a:","bv":10,"bs":100,"n":"b","v":1,"s":2}]"#,
            accept(r#"[{"bn":"a:","bv":10,"bs":100,"n":"b","v":1,"s":2}]"#),
        ),
        (
            "j-name-space",
            "4.5.1",
            N,
            "space in name",
            r#"[{"n":"temp sensor","v":1}]"#,
            Expect::Reject,
        ),
        (
            "j-name-start",
            "4.5.1",
            N,
            "name starting with `-`",
            r#"[{"n":"-temp","v":1}]"#,
            Expect::Reject,
        ),
        (
            "j-name-nonascii",
            "4.5.1",
            N,
            "non-ASCII letter in name",
            r#"[{"n":"tempé","v":1}]"#,
            Expect::Reject,
        ),
        (
            "j-name-missing",
            "4.5.1",
            N,
            "no name and no base name (empty concatenated name)",
            r#"[{"v":1}]"#,
            Expect::Reject,
        ),
        (
            "j-vd",
            "5",
            N,
            "vd unpadded base64url",
            r#"[{"n":"x","vd":"-_8"}]"#,
            accept(r#"[{"n":"x","vd":"-_8"}]"#),
        ),
        (
            "j-vd-padded",
            "5",
            N,
            "vd with `=` padding",
            r#"[{"n":"x","vd":"aGk="}]"#,
            Expect::Reject,
        ),
        (
            "j-vd-standard",
            "5",
            N,
            "vd in the standard alphabet (`+`, `/`)",
            r#"[{"n":"x","vd":"+/8"}]"#,
            Expect::Reject,
        ),
        (
            "j-dup-key",
            "5",
            R,
            "duplicate member `v` (RFC 8259 4: names SHOULD be unique; ambiguous, so rejected)",
            r#"[{"n":"x","v":1,"v":2}]"#,
            Expect::Reject,
        ),
        (
            "j-v-string",
            "5",
            N,
            "`v` as a string (Table 2: Number)",
            r#"[{"n":"x","v":"1"}]"#,
            Expect::Reject,
        ),
        (
            "j-vb-number",
            "5",
            N,
            "`vb` as a number (Table 2: Boolean)",
            r#"[{"n":"x","vb":1}]"#,
            Expect::Reject,
        ),
        (
            "j-empty",
            "11",
            N,
            "empty pack (CDDL `[1* record]`)",
            r#"[]"#,
            Expect::Reject,
        ),
        (
            "j-not-array",
            "5",
            N,
            "a record without the enclosing array",
            r#"{"n":"x","v":1}"#,
            Expect::Reject,
        ),
        (
            "j-trailing",
            "5",
            N,
            "trailing garbage after the array (not JSON)",
            r#"[{"n":"x","v":1}]x"#,
            Expect::Reject,
        ),
        (
            "j-nan",
            "5",
            N,
            "`NaN` literal (not JSON)",
            r#"[{"n":"x","v":NaN}]"#,
            Expect::Reject,
        ),
        (
            "j-1e400",
            "5",
            R,
            "1e400 is not representable as an IEEE double",
            r#"[{"n":"x","v":1e400}]"#,
            Expect::Reject,
        ),
        (
            "j-relative-time",
            "4.5.3",
            N,
            "negative relative time",
            r#"[{"n":"x","v":1,"t":-5}]"#,
            accept(r#"[{"n":"x","v":1,"t":-5}]"#),
        ),
        (
            "j-escapes",
            "5",
            N,
            "escapes in vs",
            r#"[{"n":"x","vs":"café \"q\"\n"}]"#,
            accept(r#"[{"n":"x","vs":"café \"q\"\n"}]"#),
        ),
        (
            "j-ut",
            "4.2",
            N,
            "update time",
            r#"[{"n":"x","v":1,"ut":60}]"#,
            accept(r#"[{"n":"x","v":1,"ut":60}]"#),
        ),
    ];
    for (id, section, basis, desc, input, expect) in json_cases {
        out.push(v(id, section, basis, J, desc, j(input), expect));
    }

    // The CBOR dump of section 6: the second example of 5.1.2.
    let dumps = rfc::hex_dumps("6");
    if let Some(dump) = dumps.first() {
        out.push(v(
            "rfc-6",
            "6",
            N,
            C,
            "RFC CBOR dump (second example of 5.1.2)",
            dump.clone(),
            accept(
                r#"[{"bn":"urn:dev:ow:10e2073a0108006:","bt":1.276020076001e+09,"bu":"A","bver":5,
                "n":"voltage","u":"V","v":120.1},{"n":"current","t":-5,"v":1.2},{"n":"current","t":-4,"v":1.3},
                {"n":"current","t":-3,"v":1.4},{"n":"current","t":-2,"v":1.5},{"n":"current","t":-1,"v":1.6},
                {"n":"current","t":0,"v":1.7}]"#,
            ),
        ));
    }

    let k = int;
    let forty: Vec<Vec<u8>> = (0..40)
        .map(|i| map(vec![(k(0), text(&format!("r{i}"))), (k(2), int(i))]))
        .collect();
    let forty_expect = format!(
        "[{}]",
        (0..40)
            .map(|i| format!(r#"{{"n":"r{i}","v":{i}}}"#))
            .collect::<Vec<_>>()
            .join(",")
    );
    let ok_record = map(vec![(k(0), text("x")), (k(2), int(1))]);
    let mut trailing = array(vec![ok_record.clone()]);
    trailing.push(0x00);

    let cbor_cases: Vec<(&str, &'static str, Basis, Fmt, &str, Vec<u8>, Expect)> = vec![
        (
            "c-all-labels",
            "6",
            N,
            C,
            "all 15 Table 4 labels (base ones on a base-only first record)",
            array(vec![
                map(vec![
                    (k(-1), int(10)),
                    (k(-2), text("dev:")),
                    (k(-3), f64_(1.7e9)),
                    (k(-4), text("Cel")),
                    (k(-5), int(10)),
                    (k(-6), int(100)),
                ]),
                map(vec![
                    (k(0), text("a")),
                    (k(1), text("K")),
                    (k(2), f64_(1.5)),
                    (k(5), int(2)),
                    (k(6), int(1)),
                    (k(7), int(60)),
                ]),
                map(vec![(k(0), text("b")), (k(3), text("hi"))]),
                map(vec![(k(0), text("c")), (k(4), vec![0xf5])]),
                map(vec![(k(0), text("d")), (k(8), bytes(b"hi"))]),
            ]),
            accept(
                r#"[{"bver":10,"bn":"dev:","bt":1.7e9,"bu":"Cel","bv":10,"bs":100},
                   {"n":"a","u":"K","v":1.5,"s":2,"t":1,"ut":60},{"n":"b","vs":"hi"},{"n":"c","vb":true},
                   {"n":"d","vd":"aGk"}]"#,
            ),
        ),
        (
            "c-int-time",
            "6",
            N,
            C,
            "integer time 1700000000 (4-byte uint, >= 65536)",
            one("x", vec![(k(2), int(1)), (k(6), int(1_700_000_000))]),
            accept(r#"[{"n":"x","v":1,"t":1700000000}]"#),
        ),
        (
            "c-40-records",
            "6",
            N,
            C,
            "40 records (more than 32)",
            array(forty),
            accept(&forty_expect),
        ),
        (
            "c-half",
            "6",
            N,
            C,
            "half-precision value 1.5 (f9 3e00)",
            one("x", vec![(k(2), f16_(0x3e00))]),
            accept(r#"[{"n":"x","v":1.5}]"#),
        ),
        (
            "c-single",
            "6",
            N,
            C,
            "single-precision value 23.5",
            one("x", vec![(k(2), f32_(23.5))]),
            accept(r#"[{"n":"x","v":23.5}]"#),
        ),
        (
            "c-tag4",
            "6",
            N,
            C,
            "decimal fraction 4([-1, 231]) = 23.1",
            one("x", vec![(k(2), tag(4, array(vec![int(-1), int(231)])))]),
            accept(r#"[{"n":"x","v":23.1}]"#),
        ),
        (
            "c-unknown-int-key",
            "4.4",
            N,
            C,
            "unknown integer key 9 (text value) is ignored",
            one("x", vec![(k(2), int(1)), (k(9), text("foo"))]),
            accept(r#"[{"n":"x","v":1}]"#),
        ),
        (
            "c-unknown-int-key-map",
            "4.4",
            N,
            C,
            "unknown integer key 100 with a nested map is ignored",
            one(
                "x",
                vec![(k(2), int(1)), (k(100), map(vec![(k(1), int(2))]))],
            ),
            accept(r#"[{"n":"x","v":1}]"#),
        ),
        (
            "c-text-v-only",
            "6",
            N,
            C,
            "text key \"v\" is not Value (Table 4 is conclusive): no value",
            one("x", vec![(text("v"), int(1))]),
            Expect::Reject,
        ),
        (
            "c-text-v-extra",
            "6",
            R,
            C,
            "text key \"v\" beside integer key 2: an unrecognized text label, so ignored (4.4); rejecting it is also defensible, since a CBOR-to-JSON translator would emit two \"v\" members",
            one("x", vec![(k(2), int(1)), (text("v"), int(2))]),
            accept(r#"[{"n":"x","v":1}]"#),
        ),
        (
            "c-must-understand",
            "4.4",
            N,
            C,
            "text label ending in `_`",
            one("x", vec![(k(2), int(1)), (text("foo_"), int(1))]),
            Expect::Reject,
        ),
        (
            "c-bver-float",
            "6",
            N,
            C,
            "bver as float 10.0 (only unsigned integer allowed)",
            one("x", vec![(k(-1), f64_(10.0)), (k(2), int(1))]),
            Expect::Reject,
        ),
        (
            "c-bver-11",
            "4.4",
            N,
            C,
            "bver 11",
            one("x", vec![(k(-1), int(11)), (k(2), int(1))]),
            Expect::Reject,
        ),
        (
            "c-indefinite-senml",
            "6",
            N,
            C,
            "indefinite-length array as SenML (definite MUST be used)",
            indefinite_array(vec![ok_record.clone()]),
            Expect::Reject,
        ),
        (
            "c-indefinite-sensml",
            "6",
            N,
            CS,
            "indefinite-length array as SenSML",
            indefinite_array(vec![ok_record.clone()]),
            accept(r#"[{"n":"x","v":1}]"#),
        ),
        (
            "c-two-values",
            "4.2",
            N,
            C,
            "both v (2) and vs (3)",
            one("x", vec![(k(2), int(1)), (k(3), text("a"))]),
            Expect::Reject,
        ),
        (
            "c-no-value",
            "4.2",
            N,
            C,
            "neither value nor sum",
            one("x", vec![(k(1), text("Cel"))]),
            Expect::Reject,
        ),
        (
            "c-dup-key",
            "6",
            R,
            C,
            "duplicate key 2 (not valid CBOR, RFC 7049 3.7)",
            one("x", vec![(k(2), int(1)), (k(2), int(2))]),
            Expect::Reject,
        ),
        (
            "c-vd-text",
            "6",
            N,
            C,
            "vd (8) as a text string (must be a byte string)",
            one("x", vec![(k(8), text("aGk"))]),
            Expect::Reject,
        ),
        (
            "c-vs-bytes",
            "6",
            N,
            C,
            "vs (3) as a byte string (must be a text string)",
            one("x", vec![(k(3), bytes(b"hi"))]),
            Expect::Reject,
        ),
        (
            "c-name-invalid",
            "4.5.1",
            N,
            C,
            "space in name",
            one("a b", vec![(k(2), int(1))]),
            Expect::Reject,
        ),
        (
            "c-empty",
            "11",
            N,
            C,
            "empty array (CDDL `[1* record]`)",
            array(vec![]),
            Expect::Reject,
        ),
        (
            "c-trailing",
            "6",
            N,
            C,
            "trailing byte after the array",
            trailing,
            Expect::Reject,
        ),
        (
            "c-nan",
            "6",
            R,
            C,
            "NaN value (f9 7e00): no JSON equivalent",
            one("x", vec![(k(2), f16_(0x7e00))]),
            Expect::Reject,
        ),
        (
            "c-infinity",
            "6",
            R,
            C,
            "+Infinity value (f9 7c00): no JSON equivalent",
            one("x", vec![(k(2), f16_(0x7c00))]),
            Expect::Reject,
        ),
    ];
    for (id, section, basis, fmt, desc, input, expect) in cbor_cases {
        out.push(v(id, section, basis, fmt, desc, input, expect));
    }
    out
}
