//! Cutting the excerpts in `fixtures/rfc8428` out of the full RFC 8428 text.
//!
//! The tests read only the committed excerpts. This module is for
//! `tests/extract.rs`, which checks that every excerpt is still exactly what the
//! RFC says (and can write them again). It needs the RFC from
//! `scripts/fetch-standards.sh`.

use std::path::PathBuf;

/// The cache that `scripts/fetch-standards.sh` fills with the files of
/// `standards.lock`: `$RSENML_STANDARDS`, else
/// `${XDG_CACHE_HOME:-~/.cache}/rsenml/standards`.
pub fn standards_dir() -> PathBuf {
    let var = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = var("RSENML_STANDARDS") {
        return dir;
    }
    var("XDG_CACHE_HOME")
        .or_else(|| var("HOME").map(|h| h.join(".cache")))
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("rsenml/standards")
}

/// The full RFC text from the cache, with page breaks (footer, form feed, header)
/// removed. `None` when it has not been fetched.
pub fn rfc_text() -> Option<String> {
    let raw = std::fs::read_to_string(standards_dir().join("rfc8428.txt")).ok()?;
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
    Some(out)
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

/// The figures of a section: runs of lines from one where `start` holds to one
/// where `end` holds, both included.
fn figures(body: &str, start: impl Fn(&str) -> bool, end: impl Fn(&str) -> bool) -> Vec<Vec<&str>> {
    let mut out = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in body.lines() {
        let t = line.trim();
        if current.is_none() && start(t) {
            current = Some(Vec::new());
        }
        if let Some(lines) = &mut current {
            lines.push(line);
            if end(t) {
                out.push(std::mem::take(lines));
                current = None;
            }
        }
    }
    out
}

/// The hex dumps of a section: runs of lines that start with a 4-digit hex offset.
fn dumps(body: &str) -> Vec<Vec<&str>> {
    let is_dump = |line: &str| {
        let offset = line.trim_start().split(' ').next().unwrap_or("");
        offset.len() == 4 && offset.bytes().all(|b| b.is_ascii_hexdigit())
    };
    let mut out: Vec<Vec<&str>> = Vec::new();
    let mut current = Vec::new();
    for line in body.lines() {
        if is_dump(line) {
            current.push(line);
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Lines with the indentation they share removed, each ending in a newline.
fn dedent(lines: &[&str]) -> String {
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = String::new();
    for line in lines {
        out.push_str(line.get(indent..).unwrap_or("").trim_end());
        out.push('\n');
    }
    out
}

/// A letter suffix (`a`, `b`, ...) when a section has more than one figure.
fn named(number: &str, many: bool, i: usize, ext: &str) -> String {
    if many {
        format!("{number}{}.{ext}", char::from(b'a' + i as u8))
    } else {
        format!("{number}.{ext}")
    }
}

/// Every excerpt in `fixtures/rfc8428`, as (file name, text), cut from the full
/// RFC text. `5.1.2c.txt` is the SenSML stream that the RFC leaves unfinished
/// (`...`), so it is not a JSON file.
pub fn excerpts(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for sub in 1..=7 {
        let number = format!("5.1.{sub}");
        let body = section(text, &number);
        let arrays = figures(&body, |t| t == "[", |t| t == "]" || t == "...");
        let many = arrays.len() > 1;
        for (i, lines) in arrays.iter().enumerate() {
            let partial = lines.last().is_some_and(|l| l.trim() == "...");
            let name = named(&number, many, i, if partial { "txt" } else { "json" });
            out.push((name, dedent(lines)));
        }
    }
    for number in ["6", "7", "8"] {
        let body = section(text, number);
        let xml = figures(
            &body,
            |t| t.starts_with("<sensml"),
            |t| t.ends_with("</sensml>"),
        );
        for (i, lines) in xml.iter().enumerate() {
            out.push((named(number, xml.len() > 1, i, "xml"), dedent(lines)));
        }
        let hex = dumps(&body);
        for (i, lines) in hex.iter().enumerate() {
            out.push((named(number, hex.len() > 1, i, "hex"), dedent(lines)));
        }
    }
    // The schema, as scripts/fetch-standards.sh has always cut it: blank lines
    // left by page breaks are kept, and the RFC's 3-space margin removed.
    let body = section(text, "8");
    let xsd = figures(&body, |t| t.starts_with("<?xml"), |t| t == "</xs:schema>");
    if let Some(lines) = xsd.first() {
        let mut s = String::new();
        for line in lines {
            s.push_str(line.strip_prefix("   ").unwrap_or(line));
            s.push('\n');
        }
        out.push((String::from("senml.xsd"), s));
    }
    out
}
