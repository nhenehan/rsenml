//! Test vectors for rsenml: the examples of RFC 8428, taken from the RFC text in
//! `spec/rfc8428.txt` (so every vector can be traced back to the RFC), and made-up
//! packs that cover what the examples do not.

use std::path::{Path, PathBuf};

use rsenml::{Pack, Record, Value};

/// The workspace root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// The RFC text, with page breaks (footer, form feed, header) removed.
pub fn rfc_text() -> String {
    let raw = std::fs::read_to_string(root().join("spec/rfc8428.txt")).unwrap_or_default();
    let mut out = String::new();
    for line in raw.lines() {
        let t = line.trim_start();
        if t.starts_with("Jennings, et al.")
            || t.starts_with("RFC 8428  ")
            || t.starts_with('\u{c}')
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The text of one section: from its heading to the next heading at column 0 that
/// is not one of its subsections. A heading is a line that starts with a section
/// number (digits and dots) followed by two spaces.
pub fn section(text: &str, number: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in text.lines() {
        let heading = line
            .split_once("  ")
            .filter(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
            .map(|(n, _)| n.trim_end_matches('.'));
        if let Some(n) = heading {
            if inside && !n.starts_with(&format!("{number}.")) {
                break;
            }
            if n == number {
                inside = true;
            }
        }
        if inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

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

/// Every JSON example of section 5.1, in order. An example starts at a line that is
/// only `[` and ends at a line that is only `]` (or `...`). When a section has more
/// than one, they are named with `a`, `b`, ... after the section number.
pub fn json_examples() -> Vec<JsonExample> {
    let text = rfc_text();
    let mut out = Vec::new();
    for sub in 1..=7 {
        let number = format!("5.1.{sub}");
        let body = section(&text, &number);
        let mut arrays: Vec<(String, bool)> = Vec::new();
        let mut current: Option<String> = None;
        for line in body.lines() {
            let t = line.trim();
            match &mut current {
                None if t == "[" => current = Some(String::from("[\n")),
                None => {}
                Some(buf) => {
                    if t == "]" || t == "..." {
                        let partial = t == "...";
                        buf.push(']');
                        if partial {
                            // Close the stream after the last complete record.
                            let trimmed =
                                buf.trim_end_matches(']').trim_end().trim_end_matches(',');
                            *buf = format!("{trimmed}\n]");
                        }
                        arrays.push((std::mem::take(buf), partial));
                        current = None;
                    } else {
                        buf.push_str(t);
                        buf.push('\n');
                    }
                }
            }
        }
        let many = arrays.len() > 1;
        for (i, (json, partial_stream)) in arrays.into_iter().enumerate() {
            let suffix = if many {
                String::from(char::from(b'a' + i as u8))
            } else {
                String::new()
            };
            out.push(JsonExample {
                name: format!("{number}{suffix}"),
                json,
                partial_stream,
            });
        }
    }
    out
}

/// Every `<sensml>` document in a section.
pub fn xml_examples(section_number: &str) -> Vec<String> {
    let body = section(&rfc_text(), section_number);
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("<sensml") {
            current = Some(String::new());
        }
        if let Some(buf) = &mut current {
            buf.push_str(t);
            buf.push('\n');
            if t.ends_with("</sensml>") {
                out.push(std::mem::take(buf));
                current = None;
            }
        }
    }
    out
}

/// Every hex dump in a section (`0000 a0 30 ...` lines), as bytes. A dump line starts
/// with a 4-digit hex offset. Reading stops at the first field that is not a 2-digit
/// hex byte (the ASCII column).
pub fn hex_dumps(section_number: &str) -> Vec<Vec<u8>> {
    let body = section(&rfc_text(), section_number);
    let mut out = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    for line in body.lines() {
        let t = line.trim_start();
        let mut fields = t.split(' ');
        let offset = fields.next().unwrap_or("");
        let is_dump_line = offset.len() == 4 && offset.bytes().all(|b| b.is_ascii_hexdigit());
        if is_dump_line {
            for f in fields {
                match (f.len(), u8::from_str_radix(f, 16)) {
                    (2, Ok(b)) => current.push(b),
                    _ => break,
                }
            }
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The XSD of [section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8), as
/// published (also in `spec/senml.xsd`).
pub fn rfc_xsd() -> String {
    let body = section(&rfc_text(), "8");
    let mut out = String::new();
    let mut inside = false;
    for line in body.lines() {
        if line.trim_start().starts_with("<?xml") {
            inside = true;
        }
        if inside {
            out.push_str(line.strip_prefix("   ").unwrap_or(line));
            out.push('\n');
            if line.trim() == "</xs:schema>" {
                break;
            }
        }
    }
    out
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
