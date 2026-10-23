//! RFC 8428 test vectors extracted from `spec/rfc8428.txt`, so each vector is
//! traceable to the RFC text. Copied from `crates/rsenml-tests/src/lib.rs`
//! (`rfc_text`, `section`, `json_examples`, `hex_dumps`).

use std::path::{Path, PathBuf};

/// The rsenml repository root (the parent of `compare/`).
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from(".."), Path::to_path_buf)
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

/// The text of one section: from its heading to the next heading at column 0.
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

/// A JSON example of section 5.1: its name (`5.1.2a`), text, and whether it is
/// the unfinished SenSML stream of section 5.1.2 (ending in `...`).
#[derive(Debug, Clone)]
pub struct JsonExample {
    pub name: String,
    pub json: String,
    pub partial_stream: bool,
}

/// Every JSON example of section 5.1, in order.
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

/// Every hex dump in a section (`0000 a0 30 ...` lines), as bytes.
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
