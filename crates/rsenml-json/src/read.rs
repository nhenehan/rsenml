//! Reading SenML JSON
//! ([RFC 8428 section 5](https://www.rfc-editor.org/rfc/rfc8428#section-5)).
//!
//! This file scans the outer array's brackets and the commas between records
//! itself. Each record object is parsed by nanojson's pull parser, which checks the
//! JSON grammar (strings, escapes, numbers). Two entry points share the same steps:
//! [`RecordReader`] reads complete input, and [`StreamDecoder`] reads input that
//! arrives in pieces (SenSML,
//! [section 4.8](https://www.rfc-editor.org/rfc/rfc8428#section-4.8)).

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use nanojson::{ParseError, ParseErrorKind, Parser};
use rsenml_core::codec::{FieldValue, RecordBuilder, number};
use rsenml_core::{
    Error, Format, Label, LabelType, Pack, Record, Result, ValidationOptions, Validator, base64url,
};

/// How deep arrays and objects may nest inside the value of an unknown label
/// before the reader gives up.
const MAX_DEPTH: usize = 32;

/// Reads a complete SenML JSON pack and validates it
/// ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
pub fn decode(input: &[u8]) -> Result<Pack<'static>> {
    let pack = decode_unchecked(input)?;
    pack.validate()?;
    Ok(pack)
}

/// Reads a complete SenML JSON pack, checking only the JSON form: JSON grammar,
/// label types, repeated labels, at most one value, and labels ending in `_`. Use
/// [`Pack::validate`] for the rules of section 4.
pub fn decode_unchecked(input: &[u8]) -> Result<Pack<'static>> {
    let mut reader = RecordReader::new(input);
    let mut pack = Pack::new();
    while let Some(record) = reader.next_record()? {
        pack.push(record);
    }
    reader.finish()?;
    Ok(pack)
}

/// Where the reader is in the outer array `[ record, record, ... ]`. This is a
/// small state machine: `Start` -> `[` -> `Open` -> record -> `Record` -> (`,` and
/// a record -> `Record`, or `]` -> `Done`). `Open` can also go to `Done` on `]`
/// (an empty array; the validator then reports the empty pack).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Before `[`.
    Start,
    /// After `[`, before the first record or `]`.
    Open,
    /// After a record, before `,` or `]`.
    Record,
    /// After `]`.
    Done,
}

// Returned once per record and unpacked right away. Boxing the record would add an
// allocation.
#[allow(clippy::large_enum_variant)]
enum Step {
    Record(Record<'static>),
    End,
    NeedMore,
}

/// Where a reader is in the pack. Shared by [`RecordReader`] and [`StreamDecoder`].
/// `pos` is the byte offset of the next unread byte, `index` the number of records
/// read so far, and `scratch` a buffer the parser uses to unescape strings.
#[derive(Debug, Clone)]
struct Cursor {
    pos: usize,
    state: State,
    index: usize,
    scratch: Vec<u8>,
}

impl Cursor {
    fn new() -> Self {
        Self {
            pos: 0,
            state: State::Start,
            index: 0,
            scratch: Vec::new(),
        }
    }

    /// Moves over `input` by one record. With `partial`, running out of input gives
    /// `NeedMore` (the caller adds bytes and tries again). Without it, running out is
    /// [`Error::Incomplete`]. The cursor only moves past complete records, so a
    /// retry starts again at the same record.
    fn step(&mut self, input: &[u8], partial: bool) -> Result<Step> {
        let incomplete = |partial: bool| {
            if partial {
                Ok(Step::NeedMore)
            } else {
                Err(Error::Incomplete {
                    format: Format::Json,
                })
            }
        };
        loop {
            let mut pos = skip_whitespace(input, self.pos);
            let Some(&c) = input.get(pos) else {
                return match self.state {
                    State::Done => Ok(Step::End),
                    _ => incomplete(partial),
                };
            };
            match (self.state, c) {
                (State::Start, b'[') => {
                    self.pos = pos + 1;
                    self.state = State::Open;
                }
                (State::Start, _) => return Err(syntax(pos, "a SenML pack is a JSON array")),
                (State::Open | State::Record, b']') => {
                    self.pos = pos + 1;
                    self.state = State::Done;
                    return Ok(Step::End);
                }
                (State::Record, b',') => {
                    pos = skip_whitespace(input, pos + 1);
                    match input.get(pos) {
                        None => return incomplete(partial),
                        Some(b']') => return Err(syntax(pos, "trailing comma")),
                        Some(_) => return self.record(input, pos, partial),
                    }
                }
                (State::Record, _) => return Err(syntax(pos, "expected , or ] after a record")),
                (State::Open, _) => return self.record(input, pos, partial),
                (State::Done, _) => return Err(syntax(pos, "data after the end of the pack")),
            }
        }
    }

    fn record(&mut self, input: &[u8], start: usize, partial: bool) -> Result<Step> {
        let rest = input.get(start..).unwrap_or_default();
        // When streaming, first make sure the whole object has arrived. Otherwise
        // the parser would report the cut-off object as an error. A record that is
        // not an object is left to the parser to report.
        let rest = if partial && rest.first() == Some(&b'{') {
            match object_end(rest) {
                Some(end) => rest.get(..end).unwrap_or_default(),
                None => return Ok(Step::NeedMore),
            }
        } else {
            rest
        };
        // An unescaped string is never longer than its JSON text, so a scratch
        // buffer as long as the record is always big enough.
        if self.scratch.len() < rest.len() {
            self.scratch.resize(rest.len(), 0);
        }
        let mut parser = Parser::new(rest, &mut self.scratch);
        let record = parse_record(&mut parser, self.index).map_err(|f| f.at(start))?;
        // The last token was the record's closing brace.
        self.pos = start + parser.error_offset() + 1;
        self.state = State::Record;
        self.index += 1;
        Ok(Step::Record(record))
    }

    /// Only whitespace may follow the pack.
    fn finish(&self, input: &[u8]) -> Result<()> {
        let pos = skip_whitespace(input, self.pos);
        match (self.state, input.get(pos)) {
            (State::Done, None) => Ok(()),
            (State::Done, Some(_)) => Err(syntax(pos, "data after the end of the pack")),
            _ => Err(Error::Incomplete {
                format: Format::Json,
            }),
        }
    }
}

/// Reads the records of a complete SenML JSON pack one at a time, without
/// collecting them. Use it for large packs. Records are checked as they are read
/// (JSON form only). Feed them to a [`Validator`] for the rules of section 4.
#[derive(Debug, Clone)]
pub struct RecordReader<'a> {
    input: &'a [u8],
    cursor: Cursor,
}

impl<'a> RecordReader<'a> {
    /// A reader positioned before the opening `[`.
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            cursor: Cursor::new(),
        }
    }

    /// The next record, or `None` after the closing `]`.
    pub fn next_record(&mut self) -> Result<Option<Record<'static>>> {
        match self.cursor.step(self.input, false)? {
            Step::Record(r) => Ok(Some(r)),
            Step::End | Step::NeedMore => Ok(None),
        }
    }

    /// Byte offset after the last record read (or after `]`).
    pub fn position(&self) -> usize {
        self.cursor.pos
    }

    /// Checks that the pack was closed and only whitespace follows.
    pub fn finish(&self) -> Result<()> {
        self.cursor.finish(self.input)
    }
}

impl Iterator for RecordReader<'_> {
    type Item = Result<Record<'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.next_record().transpose();
        // After an error, end the iteration: jump to the end so that later calls
        // return `None` instead of the same error again.
        if matches!(item, Some(Err(_))) {
            self.cursor.state = State::Done;
            self.cursor.pos = self.input.len();
        }
        item
    }
}

/// Decodes a SenSML JSON stream
/// ([section 4.8](https://www.rfc-editor.org/rfc/rfc8428#section-4.8)) as its bytes
/// arrive. Each record is ready as soon as its closing brace arrives.
///
/// ```
/// let mut d = rsenml_json::StreamDecoder::new();
/// d.push(br#"[{"bn":"urn:dev:ow:10e2073a01080063","bt":1.320067464e+09,"v":21.2},{"t":10,"#);
/// assert!(d.next_record().unwrap().is_some());
/// assert!(d.next_record().unwrap().is_none()); // the second record is incomplete
/// d.push(br#""v":21.3}]"#);
/// assert!(d.next_record().unwrap().is_some());
/// assert!(d.next_record().unwrap().is_none());
/// assert!(d.is_finished());
/// ```
#[derive(Debug, Clone)]
pub struct StreamDecoder {
    buf: Vec<u8>,
    cursor: Cursor,
    validator: Validator,
}

impl Default for StreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamDecoder {
    /// A decoder that validates every record
    /// ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
    pub fn new() -> Self {
        Self::with_options(ValidationOptions::default())
    }

    /// A decoder that validates every record with these options.
    pub fn with_options(options: ValidationOptions) -> Self {
        Self {
            buf: Vec::new(),
            cursor: Cursor::new(),
            validator: Validator::new(options),
        }
    }

    /// Appends received bytes.
    pub fn push(&mut self, bytes: &[u8]) {
        // Drop the bytes already read once they fill more than half of the buffer
        // (and more than 4096 bytes). This keeps the buffer from growing forever
        // without copying on every push.
        if self.cursor.pos > 4096 && self.cursor.pos * 2 > self.buf.len() {
            self.buf.drain(..self.cursor.pos);
            self.cursor.pos = 0;
        }
        self.buf.extend_from_slice(bytes);
    }

    /// The next complete record. `None` when more bytes are needed or the stream
    /// has ended ([`is_finished`](Self::is_finished)).
    pub fn next_record(&mut self) -> Result<Option<Record<'static>>> {
        match self.cursor.step(&self.buf, true)? {
            Step::Record(r) => {
                self.validator.check(&r)?;
                Ok(Some(r))
            }
            Step::End | Step::NeedMore => Ok(None),
        }
    }

    /// The closing `]` has been read.
    pub fn is_finished(&self) -> bool {
        self.cursor.state == State::Done
    }

    /// Ends the stream. It must be closed, hold at least one record, and have only
    /// whitespace after it.
    pub fn finish(&self) -> Result<()> {
        self.cursor.finish(&self.buf)?;
        self.validator.finish()
    }
}

fn skip_whitespace(input: &[u8], mut pos: usize) -> usize {
    while let Some(b' ' | b'\t' | b'\n' | b'\r') = input.get(pos) {
        pos += 1;
    }
    pos
}

/// Length of the JSON object starting at `input[0]` (a `{`), if all of it is there.
/// It only tracks strings (so brackets inside strings do not count) and nesting
/// depth. The parser checks everything else.
fn object_end(input: &[u8]) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, &c) in input.iter().enumerate() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn syntax(offset: usize, reason: &'static str) -> Error {
    Error::Syntax {
        format: Format::Json,
        offset,
        reason: reason.into(),
    }
}

/// A failure inside one record: either from the JSON parser or from the SenML
/// rules.
enum Fail {
    Json(ParseError),
    Senml(Error),
}

impl Fail {
    /// The crate error, with parser offsets made relative to the whole input. An
    /// unexpected end of input becomes [`Error::Incomplete`].
    fn at(self, start: usize) -> Error {
        match self {
            Fail::Json(ParseError {
                kind: ParseErrorKind::UnexpectedEof,
                ..
            }) => Error::Incomplete {
                format: Format::Json,
            },
            Fail::Json(e) => Error::Syntax {
                format: Format::Json,
                offset: start + e.offset,
                reason: json_reason(&e.kind).into(),
            },
            Fail::Senml(e) => e,
        }
    }
}

impl From<ParseError> for Fail {
    fn from(e: ParseError) -> Self {
        Fail::Json(e)
    }
}

impl From<Error> for Fail {
    fn from(e: Error) -> Self {
        Fail::Senml(e)
    }
}

/// The kind of JSON value ahead, or `None` when no value starts there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    String,
    Number,
    Bool,
    Null,
    Array,
    Object,
}

fn kind_ahead(p: &Parser<'_, '_>) -> Option<Kind> {
    if p.is_string_ahead() {
        Some(Kind::String)
    } else if p.is_number_ahead() {
        Some(Kind::Number)
    } else if p.is_bool_ahead() {
        Some(Kind::Bool)
    } else if p.is_null_ahead() {
        Some(Kind::Null)
    } else if p.is_array_ahead() {
        Some(Kind::Array)
    } else if p.is_object_ahead() {
        Some(Kind::Object)
    } else {
        None
    }
}

fn parse_record(
    p: &mut Parser<'_, '_>,
    index: usize,
) -> core::result::Result<Record<'static>, Fail> {
    p.object_begin()?;
    let mut b = RecordBuilder::new(index);
    while let Some(key) = p.member_decoded()? {
        match Label::from_text(key) {
            Some(label) => {
                let value = field(p, label, index)?;
                b.set(label, value)?;
            }
            None => {
                let key = String::from(key);
                extension(p, &mut b, key)?;
            }
        }
    }
    p.object_end()?;
    Ok(b.finish())
}

// The value of a registered label. It must have the JSON type of Table 2
// (RFC 8428 section 5); `vd` is a base64url string.
fn field(
    p: &mut Parser<'_, '_>,
    label: Label,
    index: usize,
) -> core::result::Result<FieldValue<'static>, Fail> {
    let Some(kind) = kind_ahead(p) else {
        // Not a value: let the parser describe what is there.
        p.null()?;
        return Err(Fail::Senml(Error::WrongType {
            record: index,
            label: label.text(),
        }));
    };
    let wrong = Fail::Senml(Error::WrongType {
        record: index,
        label: label.text(),
    });
    Ok(match (label.value_type(), kind) {
        (LabelType::String, Kind::String) => {
            FieldValue::String(Cow::Owned(p.string()?.to_string()))
        }
        (LabelType::Number, Kind::Number) => FieldValue::Number(number(p.number_str()?)),
        (LabelType::Version, Kind::Number) => {
            let text = p.number_str()?;
            // `bver` is an integer. Text like "5" parses as u64 straight away. Other
            // number text ("5.0", "5e0", "-1") goes through the double, and the
            // record builder decides whether it is a valid version.
            match text.parse::<u64>() {
                Ok(v) => FieldValue::Unsigned(v),
                Err(_) => FieldValue::Number(number(text)),
            }
        }
        (LabelType::Boolean, Kind::Bool) => FieldValue::Bool(p.boolean()?),
        (LabelType::Data, Kind::String) => {
            let data = base64url::decode(p.string()?)
                .ok_or(Fail::Senml(Error::InvalidBase64 { record: index }))?;
            FieldValue::Data(Cow::Owned(data))
        }
        _ => return Err(wrong),
    })
}

// The value of an unknown label. A string, number or boolean is kept. Anything else
// (null, array, object) is skipped, but its JSON grammar is still checked.
fn extension(
    p: &mut Parser<'_, '_>,
    b: &mut RecordBuilder<'static>,
    label: String,
) -> core::result::Result<(), Fail> {
    let value = match kind_ahead(p) {
        Some(Kind::String) => FieldValue::String(Cow::Owned(p.string()?.to_string())),
        Some(Kind::Number) => FieldValue::Number(number(p.number_str()?)),
        Some(Kind::Bool) => FieldValue::Bool(p.boolean()?),
        _ => {
            skip_value(p, 0)?;
            b.skipped_extension(&label)?;
            return Ok(());
        }
    };
    b.extension(Cow::Owned(label), value)?;
    Ok(())
}

/// Skips one value of any kind, checking its grammar.
fn skip_value(p: &mut Parser<'_, '_>, depth: usize) -> core::result::Result<(), Fail> {
    if depth > MAX_DEPTH {
        return Err(Fail::Senml(syntax(
            p.error_offset(),
            "value of an unknown label nested too deeply",
        )));
    }
    match kind_ahead(p) {
        Some(Kind::Object) => {
            p.object_begin()?;
            while p.member_decoded()?.is_some() {
                skip_value(p, depth + 1)?;
            }
            p.object_end()?;
        }
        Some(Kind::Array) => {
            p.array_begin()?;
            while p.array_item()? {
                skip_value(p, depth + 1)?;
            }
            p.array_end()?;
        }
        Some(Kind::String) => {
            p.string()?;
        }
        Some(Kind::Number) => {
            p.number_str()?;
        }
        Some(Kind::Bool) => {
            p.boolean()?;
        }
        // Null, or not a value at all: `null()` reads a null, or reports what is
        // there instead.
        Some(Kind::Null) | None => p.null()?,
    }
    Ok(())
}

/// A JSON number (already checked by the parser) as the nearest double. Numbers too
/// big for a double become infinite, and the record builder rejects them. This
/// uses `rsenml_core::number::parse_decimal` instead of `str::parse`, because
/// `str::parse` pulls `core`'s float parser and its tables into small `no_std`
/// builds.
fn number(text: &str) -> f64 {
    number::parse_decimal(text).unwrap_or(f64::NAN)
}

/// A fixed description of a JSON syntax error. (Formatting nanojson's error would
/// pull `core::fmt` into every build.)
fn json_reason(kind: &ParseErrorKind) -> &'static str {
    match kind {
        ParseErrorKind::UnexpectedToken { .. } => "unexpected token",
        ParseErrorKind::UnexpectedEof => "unexpected end of input",
        ParseErrorKind::InvalidEscape(_) => "invalid escape sequence",
        ParseErrorKind::StringBufferOverflow => "string too long",
        ParseErrorKind::InvalidUtf8 => "invalid UTF-8",
        ParseErrorKind::KeyHasEscapes => "escaped object key",
        _ => "invalid JSON",
    }
}
