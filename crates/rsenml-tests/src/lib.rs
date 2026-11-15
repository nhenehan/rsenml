//! Test vectors for rsenml: the examples of
//! [RFC 8428](https://www.rfc-editor.org/rfc/rfc8428), read from the excerpts in
//! `fixtures/rfc8428` (each cites the section it is quoted from, and
//! `tests/extract.rs` checks them against the RFC text), and made-up packs that
//! cover what the examples do not.

use std::path::{Path, PathBuf};

use rsenml::{Pack, Record, Value};

pub mod rfc_text;

/// The workspace root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// The folder of RFC 8428 excerpts.
pub fn excerpts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/rfc8428")
}

/// One excerpt of `fixtures/rfc8428`. A missing file is reported on stderr and
/// read as empty, so the tests that need it fail.
pub fn excerpt(file: &str) -> String {
    let path = excerpts_dir().join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("{}: {e}", path.display());
        String::new()
    })
}

/// The JSON examples of section 5.1, in the RFC's order. When a section has more
/// than one, they are named with `a`, `b`, ... after the section number. 5.1.2c is
/// the SenSML stream the RFC leaves unfinished.
pub const JSON_EXAMPLES: [&str; 11] = [
    "5.1.1", "5.1.2a", "5.1.2b", "5.1.2c", "5.1.3", "5.1.4", "5.1.5", "5.1.6", "5.1.7a", "5.1.7b",
    "5.1.7c",
];

/// A JSON example of
/// [section 5.1](https://www.rfc-editor.org/rfc/rfc8428#section-5.1): its name
/// (`5.1.2a`), its text, and whether it is the unfinished SenSML stream of section
/// 5.1.2 (the one ending in `...`).
#[derive(Debug, Clone)]
pub struct JsonExample {
    pub name: String,
    pub json: String,
    pub partial_stream: bool,
}

/// Every JSON example of section 5.1, in order. Each line is trimmed. The
/// unfinished stream is closed after its last complete record.
pub fn json_examples() -> Vec<JsonExample> {
    JSON_EXAMPLES
        .iter()
        .map(|name| {
            let partial_stream = *name == "5.1.2c";
            let file = format!("{name}.{}", if partial_stream { "txt" } else { "json" });
            let mut json = String::new();
            for line in excerpt(&file).lines() {
                let t = line.trim();
                if t == "..." {
                    let trimmed = json.trim_end().trim_end_matches(',');
                    json = format!("{trimmed}\n");
                    break;
                }
                json.push_str(t);
                if t != "]" {
                    json.push('\n');
                }
            }
            if partial_stream {
                json.push(']');
            }
            JsonExample {
                name: String::from(*name),
                json,
                partial_stream,
            }
        })
        .collect()
}

/// The excerpt files of one kind in a section: `7.xml`, or `8a.xml` and `8b.xml`.
fn section_files(section_number: &str, ext: &str) -> Vec<String> {
    let one = format!("{section_number}.{ext}");
    if excerpts_dir().join(&one).is_file() {
        return vec![one];
    }
    (b'a'..=b'z')
        .map(|c| format!("{section_number}{}.{ext}", char::from(c)))
        .take_while(|f| excerpts_dir().join(f).is_file())
        .collect()
}

/// Every `<sensml>` document in a section, each line trimmed.
pub fn xml_examples(section_number: &str) -> Vec<String> {
    section_files(section_number, "xml")
        .iter()
        .map(|f| {
            let mut out = String::new();
            for line in excerpt(f).lines() {
                out.push_str(line.trim());
                out.push('\n');
            }
            out
        })
        .collect()
}

/// Every hex dump in a section (`0000 a0 30 ...` lines), as bytes. Each line
/// starts with a 4-digit hex offset; reading a line stops at the first field that
/// is not a 2-digit hex byte (the ASCII column).
pub fn hex_dumps(section_number: &str) -> Vec<Vec<u8>> {
    section_files(section_number, "hex")
        .iter()
        .map(|f| {
            let mut bytes = Vec::new();
            for line in excerpt(f).lines() {
                for field in line.trim_start().split(' ').skip(1) {
                    match (field.len(), u8::from_str_radix(field, 16)) {
                        (2, Ok(b)) => bytes.push(b),
                        _ => break,
                    }
                }
            }
            bytes
        })
        .collect()
}

/// The XSD of [section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8), which
/// `scripts/gen-tables.sh` and `scripts/gen-exi-fixtures.sh` also read.
pub fn rfc_xsd() -> String {
    excerpt("senml.xsd")
}

/// Made-up packs that cover every field, value type and edge case the RFC examples
/// leave out. Each is valid SenML with no extensions (so EXI can carry it).
pub fn synthetic_cases() -> Vec<(&'static str, Pack<'static>)> {
    let all_fields = Pack::from(vec![
        Record::new()
            .with_base_name("urn:dev:ow:10e2073a01080063:")
            .with_base_time(1_700_000_000.25)
            .with_base_unit("W")
            .with_base_value(100.0)
            .with_base_sum(5000.0)
            .with_base_version(10)
            .with_name("power")
            .with_unit("W")
            .with_value(-12.5)
            .with_sum(1.5)
            .with_time(-30.0)
            .with_update_time(60.0),
        Record::new()
            .with_name("label")
            .with_string_value("Machine Room 3/B"),
        Record::new().with_name("open").with_bool_value(true),
        Record::new()
            .with_name("raw")
            .with_data_value(vec![0, 1, 2, 0xfb, 0xff, 0xfe]),
        Record::new().with_name("energy").with_sum(42.0),
    ]);
    let doubles = Pack::from(vec![
        Record::new().with_name("max").with_value(f64::MAX),
        Record::new().with_name("min").with_value(-f64::MAX),
        Record::new().with_name("tiny").with_value(5e-324),
        Record::new().with_name("third").with_value(1.0 / 3.0),
        Record::new().with_name("zero").with_value(0.0),
        Record::new().with_name("negzero").with_value(-0.0),
        Record::new()
            .with_name("int")
            .with_value(9_007_199_254_740_993.0),
        Record::new().with_name("big").with_value(1e21),
        Record::new().with_name("small").with_value(1e-7),
    ]);
    let unicode = Pack::from(vec![
        Record::new()
            .with_name("text")
            .with_string_value("\u{e9}t\u{e9} \u{65e5}\u{672c} \u{1f321}\u{fe0f} \"q\" <a&b> \\ /"),
        Record::new()
            .with_name("lines")
            .with_string_value("a\tb\nc\r\nd"),
    ]);
    let many: Pack = (0..200)
        .map(|i| {
            let mut r = Record::new()
                .with_time(1_700_000_000.0 + f64::from(i))
                .with_value(20.0 + f64::from(i % 17) / 10.0);
            if i == 0 {
                r = r
                    .with_base_name("urn:dev:mac:0024befffe804ff1/")
                    .with_name("temp")
                    .with_unit("Cel");
            } else {
                r = r.with_name("temp");
            }
            r
        })
        .collect();
    let base_only = Pack::from(vec![
        Record::new()
            .with_base_name("dev/")
            .with_base_time(1_700_000_000.0),
        Record::new().with_name("a").with_value(1.0),
        Record::new().with_base_unit("V"),
        Record::new().with_name("b").with_value(2.0),
    ]);
    vec![
        ("all-fields", all_fields),
        ("doubles", doubles),
        ("unicode", unicode),
        ("many-records", many),
        ("base-only-records", base_only),
    ]
}

/// The RFC's JSON examples (those that decode as complete packs) and the made-up
/// cases, as packs.
pub fn all_cases() -> Vec<(String, Pack<'static>)> {
    let mut out: Vec<(String, Pack<'static>)> = json_examples()
        .into_iter()
        .filter_map(|ex| {
            rsenml::json::decode(ex.json.as_bytes())
                .ok()
                .map(|p| (format!("rfc-{}", ex.name), p))
        })
        .collect();
    out.extend(
        synthetic_cases()
            .into_iter()
            .map(|(n, p)| (String::from(n), p)),
    );
    out
}

/// Equality of packs that treats numbers by bit pattern (so -0.0 != 0.0).
pub fn same_pack(a: &Pack<'_>, b: &Pack<'_>) -> bool {
    a == b
        && a.iter().zip(b).all(|(x, y)| {
            x.numbers()
                .zip(y.numbers())
                .all(|((_, u), (_, v))| u.to_bits() == v.to_bits())
        })
}

/// The pack as EXI can carry it. EXI floats have no negative zero: an EXI Float's
/// mantissa is an EXI Integer
/// ([EXI 7.1.4](https://www.w3.org/TR/exi/#encodingFloat)), and
/// [EXI 7.1.5](https://www.w3.org/TR/exi/#encodingInteger) codes a negative integer
/// as its magnitude minus one, so the mantissa of -0.0 is 0. So -0.0 comes back as
/// 0.0.
pub fn exi_view(pack: &Pack<'static>) -> Pack<'static> {
    // `-0.0 == 0.0` is true, so this turns -0.0 into 0.0 and leaves other numbers.
    let fix = |v: &mut f64| {
        if *v == 0.0 {
            *v = 0.0;
        }
    };
    let mut out = pack.clone();
    for r in &mut out.records {
        for f in [
            &mut r.base_time,
            &mut r.base_value,
            &mut r.base_sum,
            &mut r.sum,
            &mut r.time,
            &mut r.update_time,
        ]
        .into_iter()
        .flatten()
        {
            fix(f);
        }
        if let Some(Value::Number(v)) = &mut r.value {
            fix(v);
        }
    }
    out
}

/// Shorthand for a value of a record, for assertions.
pub fn value_of<'a>(pack: &'a Pack<'_>, i: usize) -> Option<&'a Value<'a>> {
    pack.records.get(i).and_then(|r| r.value.as_ref())
}
