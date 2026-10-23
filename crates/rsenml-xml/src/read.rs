//! Reading SenML XML
//! ([RFC 8428 section 7](https://www.rfc-editor.org/rfc/rfc8428#section-7)).
//!
//! The document is a `sensml` element in the namespace
//! `urn:ietf:params:xml:ns:senml`. It holds `senml` elements, one per record. Their
//! attributes without a prefix are the fields (Table 5). xmlparser splits the text
//! into tokens. This module does the rest: it resolves namespaces
//! ([Namespaces in XML](https://www.w3.org/TR/xml-names/)), matches end tags,
//! expands character references and the predefined entity references, and
//! normalizes attribute values
//! ([XML 1.0 section 3.3.3](https://www.w3.org/TR/xml/#AVNormalize)). Then it applies
//! the XSD types of the RFC's schema.
//!
//! - Only UTF-8 is accepted (section 7). A DTD is rejected.
//! - Whitespace, comments and processing instructions may appear between records.
//!   Other text and child elements may not.
//! - Attributes in other namespaces (`xml:lang`, `xmlns:*`, foreign ones) are
//!   ignored. Unknown attributes without a prefix are extensions (string values).
//! - `INF`, `-INF` and `NaN` are valid `xs:double` values but not SenML numbers.

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use rsenml_core::codec::{FieldValue, RecordBuilder, number};
use rsenml_core::{
    Error, Format, Label, LabelType, Pack, Record, Result, XML_NAMESPACE, base64url,
};
use xmlparser::{ElementEnd, StrSpan, Token, Tokenizer};

const XML_PREFIX_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// Reads a SenML XML document and validates the pack
/// ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
pub fn decode(input: &[u8]) -> Result<Pack<'_>> {
    let pack = decode_unchecked(input)?;
    pack.validate()?;
    Ok(pack)
}

/// Reads a SenML XML document, checking only the XML form (not the rules of
/// section 4).
pub fn decode_unchecked(input: &[u8]) -> Result<Pack<'_>> {
    let mut reader = RecordReader::new(input)?;
    let mut pack = Pack::new();
    while let Some(record) = reader.next_record()? {
        pack.push(record);
    }
    reader.finish()?;
    Ok(pack)
}

/// Where the reader is in the document. `Prolog` -> (root start tag) -> `Records`
/// -> (`</sensml>`) -> `Epilog`. An empty root `<sensml/>` goes straight from
/// `Prolog` to `Epilog`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Before the root element.
    Prolog,
    /// Inside `sensml`.
    Records,
    /// After `</sensml>`.
    Epilog,
}

/// Reads the records of a SenML XML document one at a time. Attribute values with
/// no references and no tab, newline or carriage return borrow from the input.
///
/// Records have no child elements, so namespaces only need two levels: the root's
/// declarations (kept here) and each `senml` element's own.
#[derive(Clone)]
pub struct RecordReader<'a> {
    text: &'a str,
    tokens: Tokenizer<'a>,
    state: State,
    index: usize,
    /// The root's namespace declarations: the default namespace, then the ones with
    /// a prefix, as (prefix, URI) pairs. `root_prefix` is the root's own prefix,
    /// needed to match `</sensml>`.
    root_default: Option<&'a str>,
    root_prefixes: Vec<(&'a str, &'a str)>,
    root_prefix: &'a str,
}

impl core::fmt::Debug for RecordReader<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RecordReader")
            .field("state", &self.state)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

/// A start tag with its attributes, collected before namespaces are resolved
/// (an `xmlns` attribute may come after the attributes it applies to). Each
/// attribute is (prefix, local name, raw value).
struct StartTag<'a> {
    prefix: &'a str,
    local: &'a str,
    offset: usize,
    attributes: Vec<(StrSpan<'a>, StrSpan<'a>, StrSpan<'a>)>,
    empty: bool,
}

impl<'a> RecordReader<'a> {
    /// Fails if `input` is not UTF-8.
    pub fn new(input: &'a [u8]) -> Result<Self> {
        let text = core::str::from_utf8(input).map_err(|e| Error::Syntax {
            format: Format::Xml,
            offset: e.valid_up_to(),
            reason: "SenML XML must be UTF-8".into(),
        })?;
        Ok(Self {
            text,
            tokens: Tokenizer::from(text),
            state: State::Prolog,
            index: 0,
            root_default: None,
            root_prefixes: Vec::new(),
            root_prefix: "",
        })
    }

    /// The next record, or `None` at `</sensml>`.
    pub fn next_record(&mut self) -> Result<Option<Record<'a>>> {
        loop {
            let Some(token) = self.next_token()? else {
                return Err(match self.state {
                    State::Prolog => structure("no sensml element"),
                    State::Records => Error::Incomplete {
                        format: Format::Xml,
                    },
                    State::Epilog => return Ok(None),
                });
            };
            match (self.state, token) {
                (_, Token::Comment { .. } | Token::ProcessingInstruction { .. }) => {}
                (State::Prolog, Token::Declaration { encoding, .. }) => {
                    if let Some(enc) = encoding
                        && !enc.as_str().eq_ignore_ascii_case("utf-8")
                    {
                        return Err(syntax(enc.start(), "SenML XML must be UTF-8"));
                    }
                }
                (
                    _,
                    Token::DtdStart { span, .. }
                    | Token::EmptyDtd { span, .. }
                    | Token::EntityDeclaration { span, .. },
                ) => {
                    return Err(syntax(span.start(), "a DTD is not allowed in SenML XML"));
                }
                (_, Token::Text { text }) => self.whitespace_only(text)?,
                (_, Token::Cdata { text, span }) => {
                    if !is_whitespace(text.as_str()) {
                        return Err(syntax(span.start(), "text is not allowed here"));
                    }
                }
                (
                    State::Prolog,
                    Token::ElementStart {
                        prefix,
                        local,
                        span,
                    },
                ) => {
                    let tag = self.start_tag(prefix, local, span)?;
                    self.root(tag)?;
                    if self.state == State::Epilog {
                        return Ok(None);
                    }
                }
                (
                    State::Records,
                    Token::ElementStart {
                        prefix,
                        local,
                        span,
                    },
                ) => {
                    let tag = self.start_tag(prefix, local, span)?;
                    let record = self.record(tag)?;
                    return Ok(Some(record));
                }
                (State::Records, Token::ElementEnd { end, span }) => {
                    self.end_tag(end, self.root_prefix, "sensml", span)?;
                    self.state = State::Epilog;
                    return Ok(None);
                }
                (State::Epilog, Token::ElementStart { span, .. }) => {
                    return Err(syntax(span.start(), "only one sensml element is allowed"));
                }
                (_, token) => return Err(syntax(token_start(&token), "unexpected markup")),
            }
        }
    }

    /// Checks that only comments, processing instructions and whitespace follow
    /// the root element.
    pub fn finish(&mut self) -> Result<()> {
        if self.state != State::Epilog {
            return Err(Error::Incomplete {
                format: Format::Xml,
            });
        }
        while let Some(token) = self.next_token()? {
            match token {
                Token::Comment { .. } | Token::ProcessingInstruction { .. } => {}
                Token::Text { text } => self.whitespace_only(text)?,
                other => return Err(syntax(token_start(&other), "data after the sensml element")),
            }
        }
        Ok(())
    }

    fn next_token(&mut self) -> Result<Option<Token<'a>>> {
        match self.tokens.next() {
            None => Ok(None),
            Some(Ok(t)) => Ok(Some(t)),
            Some(Err(e)) => {
                let pos = e.pos();
                let offset = offset_of(self.text, pos.row, pos.col);
                Err(if offset >= self.text.len() {
                    Error::Incomplete {
                        format: Format::Xml,
                    }
                } else {
                    Error::Syntax {
                        format: Format::Xml,
                        offset,
                        reason: xml_reason(&e).into(),
                    }
                })
            }
        }
    }

    fn whitespace_only(&self, text: StrSpan<'_>) -> Result<()> {
        if is_whitespace(text.as_str()) {
            Ok(())
        } else {
            Err(syntax(text.start(), "text is not allowed here"))
        }
    }

    fn start_tag(
        &mut self,
        prefix: StrSpan<'a>,
        local: StrSpan<'a>,
        span: StrSpan<'a>,
    ) -> Result<StartTag<'a>> {
        let mut tag = StartTag {
            prefix: prefix.as_str(),
            local: local.as_str(),
            offset: span.start(),
            attributes: Vec::new(),
            empty: false,
        };
        loop {
            match self.next_token()? {
                Some(Token::Attribute {
                    prefix,
                    local,
                    value,
                    ..
                }) => tag.attributes.push((prefix, local, value)),
                Some(Token::ElementEnd {
                    end: ElementEnd::Open,
                    ..
                }) => return Ok(tag),
                Some(Token::ElementEnd {
                    end: ElementEnd::Empty,
                    ..
                }) => {
                    tag.empty = true;
                    return Ok(tag);
                }
                Some(other) => return Err(syntax(token_start(&other), "malformed start tag")),
                None => {
                    return Err(Error::Incomplete {
                        format: Format::Xml,
                    });
                }
            }
        }
    }

    /// The namespace URI of `prefix`. The element's own declarations come first,
    /// then the root's. The `xml` prefix is always bound to its fixed URI.
    fn namespace(&self, tag: &StartTag<'a>, prefix: &str) -> Option<&'a str> {
        let own = tag.attributes.iter().find_map(|(p, l, v)| {
            let declares = if prefix.is_empty() {
                p.as_str().is_empty() && l.as_str() == "xmlns"
            } else {
                p.as_str() == "xmlns" && l.as_str() == prefix
            };
            declares.then(|| v.as_str())
        });
        own.or(if prefix.is_empty() {
            self.root_default
        } else if prefix == "xml" {
            Some(XML_PREFIX_NAMESPACE)
        } else {
            self.root_prefixes
                .iter()
                .find_map(|&(p, uri)| (p == prefix).then_some(uri))
        })
    }

    fn root(&mut self, tag: StartTag<'a>) -> Result<()> {
        if tag.local != "sensml" || self.namespace(&tag, tag.prefix) != Some(XML_NAMESPACE) {
            return Err(structure(
                "the root element must be sensml in urn:ietf:params:xml:ns:senml",
            ));
        }
        for (p, l, v) in &tag.attributes {
            match (p.as_str(), l.as_str()) {
                ("", "xmlns") => self.root_default = Some(v.as_str()),
                ("xmlns", prefix) => self.root_prefixes.push((prefix, v.as_str())),
                _ => {}
            }
        }
        self.root_prefix = tag.prefix;
        // `<sensml/>`: a document with no records (validation reports that).
        self.state = if tag.empty {
            State::Epilog
        } else {
            State::Records
        };
        Ok(())
    }

    fn record(&mut self, tag: StartTag<'a>) -> Result<Record<'a>> {
        if tag.local != "senml" || self.namespace(&tag, tag.prefix) != Some(XML_NAMESPACE) {
            return Err(Error::Syntax {
                format: Format::Xml,
                offset: tag.offset,
                reason: "only senml elements may appear inside sensml".into(),
            });
        }
        let index = self.index;
        self.index += 1;
        let mut b = RecordBuilder::new(index);
        for (prefix, local, value) in &tag.attributes {
            // Skip namespace declarations and attributes with a prefix. In XML, an
            // attribute without a prefix is in no namespace, which is where the
            // SenML labels are.
            if !prefix.as_str().is_empty() || local.as_str() == "xmlns" {
                continue;
            }
            let text = attribute_value(*value)?;
            let label = local.as_str();
            match Label::from_text(label) {
                Some(label) => b.set(label, typed(label, text, index)?)?,
                None => b.extension(Cow::Borrowed(label), FieldValue::String(text))?,
            }
        }
        if !tag.empty {
            // The content must be empty (whitespace, comments and processing
            // instructions are allowed).
            loop {
                match self.next_token()? {
                    Some(Token::Comment { .. } | Token::ProcessingInstruction { .. }) => {}
                    Some(Token::Text { text }) => self.whitespace_only(text)?,
                    Some(Token::ElementEnd { end, span }) => {
                        self.end_tag(end, tag.prefix, "senml", span)?;
                        break;
                    }
                    Some(other) => {
                        return Err(syntax(
                            token_start(&other),
                            "a senml element has no content",
                        ));
                    }
                    None => {
                        return Err(Error::Incomplete {
                            format: Format::Xml,
                        });
                    }
                }
            }
        }
        Ok(b.finish())
    }

    fn end_tag(
        &self,
        end: ElementEnd<'_>,
        prefix: &str,
        local: &str,
        span: StrSpan<'_>,
    ) -> Result<()> {
        match end {
            ElementEnd::Close(p, l) if p.as_str() == prefix && l.as_str() == local => Ok(()),
            _ => Err(syntax(span.start(), "end tag does not match its start tag")),
        }
    }
}

fn is_whitespace(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

fn syntax(offset: usize, reason: &'static str) -> Error {
    Error::Syntax {
        format: Format::Xml,
        offset,
        reason: reason.into(),
    }
}

fn structure(reason: &'static str) -> Error {
    Error::Structure {
        format: Format::Xml,
        reason,
    }
}

fn token_start(token: &Token<'_>) -> usize {
    match token {
        Token::Declaration { span, .. }
        | Token::ProcessingInstruction { span, .. }
        | Token::Comment { span, .. }
        | Token::DtdStart { span, .. }
        | Token::EmptyDtd { span, .. }
        | Token::EntityDeclaration { span, .. }
        | Token::DtdEnd { span }
        | Token::ElementStart { span, .. }
        | Token::Attribute { span, .. }
        | Token::ElementEnd { span, .. }
        | Token::Cdata { span, .. } => span.start(),
        Token::Text { text } => text.start(),
    }
}

/// Byte offset of a row and column (both start at 1; the column counts characters)
/// in `text`. xmlparser reports errors this way.
fn offset_of(text: &str, row: u32, col: u32) -> usize {
    let mut offset = 0;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        if i + 1 == row as usize {
            return offset
                + line
                    .char_indices()
                    .nth((col as usize).saturating_sub(1))
                    .map_or(line.len(), |(o, _)| o);
        }
        offset += line.len();
    }
    text.len()
}

/// The normalized value of an attribute
/// ([XML 1.0 section 3.3.3](https://www.w3.org/TR/xml/#AVNormalize)): references
/// expanded, and each literal tab, newline and carriage return (or CR LF pair)
/// replaced by a space. A tab or newline written as a character reference stays as
/// it is, as XML requires.
fn attribute_value(raw: StrSpan<'_>) -> Result<Cow<'_, str>> {
    let s = raw.as_str();
    if !s.bytes().any(|b| matches!(b, b'&' | b'\t' | b'\n' | b'\r')) {
        return Ok(Cow::Borrowed(s));
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        match c {
            '&' => {
                let end = rest
                    .find(';')
                    .ok_or_else(|| syntax(raw.start(), "unterminated reference"))?;
                let name = &rest[1..end];
                out.push(
                    reference(name)
                        .ok_or_else(|| syntax(raw.start(), "unknown or invalid reference"))?,
                );
                rest = &rest[end + 1..];
            }
            '\r' => {
                out.push(' ');
                rest = rest.strip_prefix("\r\n").unwrap_or(&rest[1..]);
            }
            '\t' | '\n' => {
                out.push(' ');
                rest = &rest[1..];
            }
            c => {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    Ok(Cow::Owned(out))
}

/// A predefined entity or a character reference (`lt`, `#38`, `#x26`). `name` is
/// the text between `&` and `;`.
fn reference(name: &str) -> Option<char> {
    match name {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let code = if let Some(hex) = name.strip_prefix("#x") {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                name.strip_prefix('#')?.parse().ok()?
            };
            let c = char::from_u32(code)?;
            // XML 1.0 Char production (https://www.w3.org/TR/xml/#NT-Char): tab,
            // newline, carriage return, and everything from space up except U+FFFE
            // and U+FFFF. (`char` already rules out surrogates.)
            (matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}'))
                .then_some(c)
        }
    }
}

/// The value of a registered label, by its XML type (Table 5 in
/// [section 7](https://www.rfc-editor.org/rfc/rfc8428#section-7)).
fn typed<'a>(label: Label, text: Cow<'a, str>, index: usize) -> Result<FieldValue<'a>> {
    let wrong = || Error::WrongType {
        record: index,
        label: label.text(),
    };
    Ok(match label.value_type() {
        LabelType::String => FieldValue::String(text),
        LabelType::Number => FieldValue::Number(xs_double(collapse(&text), label, index)?),
        LabelType::Version => {
            let t = collapse(&text);
            let digits = t.strip_prefix('+').unwrap_or(t);
            // `bver` is an `xs:int`. A well-formed negative number is a bad version;
            // anything else that is not digits is the wrong type.
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(
                    if t.starts_with('-')
                        && t.len() > 1
                        && t[1..].bytes().all(|b| b.is_ascii_digit())
                    {
                        Error::InvalidVersion { record: index }
                    } else {
                        wrong()
                    },
                );
            }
            // xs:int: at most 2147483647 (https://www.w3.org/TR/xmlschema-2/#int).
            match digits.parse::<u64>() {
                Ok(v) if v <= 2_147_483_647 => FieldValue::Unsigned(v),
                _ => return Err(wrong()),
            }
        }
        // xs:boolean allows "true", "false", "1" and "0"
        // (https://www.w3.org/TR/xmlschema-2/#boolean).
        LabelType::Boolean => match collapse(&text) {
            "true" | "1" => FieldValue::Bool(true),
            "false" | "0" => FieldValue::Bool(false),
            _ => return Err(wrong()),
        },
        LabelType::Data => FieldValue::Data(Cow::Owned(
            base64url::decode(&text).ok_or(Error::InvalidBase64 { record: index })?,
        )),
    })
}

/// XSD whiteSpace="collapse"
/// ([XML Schema 2](https://www.w3.org/TR/xmlschema-2/#rf-whiteSpace)) for a single
/// token: leading and trailing spaces are removed. Attribute normalization has
/// already turned tabs and newlines into spaces.
fn collapse(s: &str) -> &str {
    s.trim_matches(' ')
}

/// An `xs:double` value in its text form
/// ([XML Schema 2](https://www.w3.org/TR/xmlschema-2/#double)). The special values
/// (`INF`, `-INF`, `NaN`) are not SenML numbers.
fn xs_double(t: &str, label: Label, index: usize) -> Result<f64> {
    if matches!(t, "INF" | "+INF" | "-INF" | "NaN") {
        return Err(Error::NonFinite {
            record: index,
            label: label.text().to_string(),
        });
    }
    // Check the syntax by hand against this pattern, then parse:
    // (\+|-)?([0-9]+(\.[0-9]*)?|\.[0-9]+)([Ee](\+|-)?[0-9]+)?
    let b = t.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let int_start = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let mut digits = i - int_start;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let frac_start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        digits += i - frac_start;
    }
    if digits > 0 && matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let exp_start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == exp_start {
            digits = 0;
        }
    }
    if digits == 0 || i != b.len() {
        return Err(Error::WrongType {
            record: index,
            label: label.text(),
        });
    }
    let v = number::parse_decimal(t).ok_or(Error::WrongType {
        record: index,
        label: label.text(),
    })?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(Error::NonFinite {
            record: index,
            label: label.text().to_string(),
        })
    }
}

/// A fixed description of an XML syntax error. (Formatting xmlparser's error would
/// pull `core::fmt` into every build.)
fn xml_reason(e: &xmlparser::Error) -> &'static str {
    match e {
        xmlparser::Error::InvalidDeclaration(..) => "invalid XML declaration",
        xmlparser::Error::InvalidComment(..) => "invalid comment",
        xmlparser::Error::InvalidPI(..) => "invalid processing instruction",
        xmlparser::Error::InvalidDoctype(..) => "invalid DOCTYPE",
        xmlparser::Error::InvalidEntity(..) => "invalid entity",
        xmlparser::Error::InvalidElement(..) => "invalid element",
        xmlparser::Error::InvalidAttribute(..) => "invalid attribute",
        xmlparser::Error::InvalidCdata(..) => "invalid CDATA section",
        xmlparser::Error::InvalidCharData(..) => "invalid character data",
        xmlparser::Error::UnknownToken(..) => "unknown token",
    }
}
