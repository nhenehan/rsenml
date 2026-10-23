//! Writing SenML XML
//! ([RFC 8428 section 7](https://www.rfc-editor.org/rfc/rfc8428#section-7)).
//!
//! The output is `<sensml xmlns="urn:ietf:params:xml:ns:senml">` with one empty
//! `<senml .../>` element per record (the RFC example's `<senml ...></senml>` is the
//! same XML). Attributes come in [`Label::WRITE_ORDER`], with extensions after them.
//! No XML declaration and no whitespace are written. Attribute values escape `&`,
//! `<` and `"`. They also escape tab, newline and carriage return, so that these
//! survive attribute-value normalization
//! ([XML 1.0 section 3.3.3](https://www.w3.org/TR/xml/#AVNormalize)), which would
//! turn them into spaces.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

use rsenml_core::codec::number;
use rsenml_core::{Error, Format, Label, Pack, Record, Result, Value, XML_NAMESPACE, base64url};

/// Writes a pack after validating it
/// ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
pub fn encode(pack: &Pack<'_>) -> Result<Vec<u8>> {
    pack.validate()?;
    encode_unchecked(pack)
}

/// Writes a pack as it is, without validating (values XML cannot carry are still
/// refused).
pub fn encode_unchecked(pack: &Pack<'_>) -> Result<Vec<u8>> {
    let mut out = String::new();
    write_pack(&mut out, pack)?;
    Ok(out.into_bytes())
}

/// Validates a pack and writes it into `buf`, without allocating for the output.
/// Returns the length written, or [`Error::BufferFull`].
pub fn encode_into(pack: &Pack<'_>, buf: &mut [u8]) -> Result<usize> {
    pack.validate()?;
    let mut out = SliceOut { buf, len: 0 };
    write_pack(&mut out, pack)?;
    Ok(out.len)
}

/// Appends one `<senml .../>` element, for writing a SenSML document one piece at a
/// time. See [`StreamEncoder`].
pub fn encode_record(record: &Record<'_>, index: usize, out: &mut Vec<u8>) -> Result<()> {
    let mut text = String::new();
    write_record(&mut text, record, index)?;
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

/// Writes a SenSML XML document one piece at a time: the `sensml` start tag before
/// the first record, one `senml` element per record, and the end tag in
/// [`finish`](Self::finish). It does not validate.
#[derive(Debug, Clone, Default)]
pub struct StreamEncoder {
    records: usize,
}

impl StreamEncoder {
    /// An encoder that has written nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends the next record (with the `sensml` start tag before the first).
    pub fn record(&mut self, record: &Record<'_>, out: &mut Vec<u8>) -> Result<()> {
        if self.records == 0 {
            start_bytes(out);
        }
        encode_record(record, self.records, out)?;
        self.records += 1;
        Ok(())
    }

    /// Appends the end tag (or both the `sensml` start tag and end tag, if no record
    /// was written).
    pub fn finish(self, out: &mut Vec<u8>) {
        if self.records == 0 {
            start_bytes(out);
        }
        out.extend_from_slice(b"</sensml>");
    }
}

/// `<sensml xmlns="urn:ietf:params:xml:ns:senml">`
fn write_start<W: Write>(out: &mut W) -> core::fmt::Result {
    out.write_str("<sensml xmlns=\"")?;
    out.write_str(XML_NAMESPACE)?;
    out.write_str("\">")
}

/// [`write_start`] onto a byte vector.
fn start_bytes(out: &mut Vec<u8>) {
    let mut s = String::new();
    // Writing to a String cannot fail.
    let _ = write_start(&mut s);
    out.extend_from_slice(s.as_bytes());
}

/// A fixed buffer as a `core::fmt::Write`. When it is full, the write fails, which
/// becomes [`Error::BufferFull`].
struct SliceOut<'b> {
    buf: &'b mut [u8],
    len: usize,
}

impl Write for SliceOut<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        let dst = self.buf.get_mut(self.len..end).ok_or(core::fmt::Error)?;
        dst.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// Only the fixed buffer fails, and only when full.
fn full(_: core::fmt::Error) -> Error {
    Error::BufferFull
}

fn write_pack<W: Write>(out: &mut W, pack: &Pack<'_>) -> Result<()> {
    write_start(out).map_err(full)?;
    for (i, r) in pack.iter().enumerate() {
        write_record(out, r, i)?;
    }
    out.write_str("</sensml>").map_err(full)
}

fn write_record<W: Write>(out: &mut W, r: &Record<'_>, index: usize) -> Result<()> {
    out.write_str("<senml").map_err(full)?;
    for label in Label::WRITE_ORDER {
        let text = label.text();
        match label {
            Label::BaseName => string(out, text, r.base_name.as_deref())?,
            Label::BaseUnit => string(out, text, r.base_unit.as_deref())?,
            Label::Name => string(out, text, r.name.as_deref())?,
            Label::Unit => string(out, text, r.unit.as_deref())?,
            Label::BaseTime => num(out, text, r.base_time, index)?,
            Label::BaseValue => num(out, text, r.base_value, index)?,
            Label::BaseSum => num(out, text, r.base_sum, index)?,
            Label::Time => num(out, text, r.time, index)?,
            Label::UpdateTime => num(out, text, r.update_time, index)?,
            Label::Sum => num(out, text, r.sum, index)?,
            Label::BaseVersion => {
                if let Some(v) = r.base_version {
                    // `bver` is an xs:int (https://www.w3.org/TR/xmlschema-2/#int).
                    if v > 2_147_483_647 {
                        return Err(unrepresentable("bver above the xs:int range"));
                    }
                    write!(out, " bver=\"{v}\"").map_err(full)?;
                }
            }
            // The record has at most one value. Write it at the place of its own
            // label in the order.
            Label::Value | Label::StringValue | Label::BoolValue | Label::DataValue => {
                if let Some(v) = r.value.as_ref().filter(|v| v.label() == label) {
                    value(out, text, v, index)?;
                }
            }
        }
    }
    for ext in &r.extensions {
        if !is_ncname(&ext.label) {
            return Err(unrepresentable(
                "an extension label that is not an XML name",
            ));
        }
        value(out, &ext.label, &ext.value, index)?;
    }
    out.write_str("/>").map_err(full)
}

fn unrepresentable(reason: &'static str) -> Error {
    Error::Unrepresentable {
        format: Format::Xml,
        reason: reason.into(),
    }
}

fn string<W: Write>(out: &mut W, label: &str, v: Option<&str>) -> Result<()> {
    match v {
        Some(v) => attribute(out, label, v),
        None => Ok(()),
    }
}

fn num<W: Write>(out: &mut W, label: &str, v: Option<f64>, index: usize) -> Result<()> {
    match v {
        Some(v) => number_attribute(out, label, v, index),
        None => Ok(()),
    }
}

fn number_attribute<W: Write>(out: &mut W, label: &str, v: f64, index: usize) -> Result<()> {
    if !v.is_finite() {
        return Err(Error::NonFinite {
            record: index,
            label: label.to_string(),
        });
    }
    write!(out, " {label}=\"").map_err(full)?;
    number::write_number_to(out, v).map_err(full)?;
    out.write_char('"').map_err(full)
}

fn value<W: Write>(out: &mut W, label: &str, v: &Value<'_>, index: usize) -> Result<()> {
    match v {
        Value::Number(v) => number_attribute(out, label, *v, index),
        Value::String(s) => attribute(out, label, s),
        Value::Bool(b) => attribute(out, label, if *b { "true" } else { "false" }),
        Value::Data(d) => {
            // base64url needs no escaping.
            write!(out, " {label}=\"").map_err(full)?;
            base64url::encode_to(d, out).map_err(full)?;
            out.write_char('"').map_err(full)
        }
    }
}

fn attribute<W: Write>(out: &mut W, label: &str, value: &str) -> Result<()> {
    write!(out, " {label}=\"").map_err(full)?;
    for c in value.chars() {
        let r = match c {
            '&' => out.write_str("&amp;"),
            '<' => out.write_str("&lt;"),
            '"' => out.write_str("&quot;"),
            '\t' => out.write_str("&#9;"),
            '\n' => out.write_str("&#10;"),
            '\r' => out.write_str("&#13;"),
            // Other control characters, U+FFFE and U+FFFF are not XML 1.0 characters
            // (https://www.w3.org/TR/xml/#NT-Char), not even as references.
            c if c < ' ' || c == '\u{fffe}' || c == '\u{ffff}' => {
                return Err(unrepresentable("a character XML 1.0 does not allow"));
            }
            c => out.write_char(c),
        };
        r.map_err(full)?;
    }
    out.write_char('"').map_err(full)
}

/// An XML NCName (a name with no colon,
/// [Namespaces in XML](https://www.w3.org/TR/xml-names/#NT-NCName)), limited to
/// what SenML labels use: ASCII letters, digits, `-`, `.` and `_`, starting with a
/// letter or `_`.
fn is_ncname(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(is_name_start_char) && chars.all(is_name_char)
}

/// A character that may start an XML name, without the colon (names here have no
/// prefix). The decoder accepts all of these, so the encoder must too, or a pack it
/// read could not be written back.
/// [XML 1.0 NameStartChar](https://www.w3.org/TR/xml/#NT-NameStartChar).
fn is_name_start_char(c: char) -> bool {
    matches!(c,
        'A'..='Z' | '_' | 'a'..='z'
        | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}'
        | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}' | '\u{200c}'..='\u{200d}'
        | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}'
        | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
}

/// A character that may follow the first one in an XML name.
/// [XML 1.0 NameChar](https://www.w3.org/TR/xml/#NT-NameChar).
fn is_name_char(c: char) -> bool {
    is_name_start_char(c)
        || matches!(c,
            '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}')
}
