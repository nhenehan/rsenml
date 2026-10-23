//! Reading SenML CBOR
//! ([RFC 8428 section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)).
//!
//! - Registered labels are the integer keys of Table 4 (in section 6). A text key
//!   that spells a registered label (`"v"`) is not that label in CBOR. So it is an
//!   unknown field and is ignored
//!   ([section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)). Its
//!   value is skipped, not kept, so that a translation to JSON cannot write the
//!   label twice.
//! - Unknown integer keys are ignored. Unknown text keys are extensions (kept when
//!   their value is a scalar), and fail when they end in `_`.
//! - Numbers may be integers, half/single/double floats, or decimal fractions
//!   (tag 4,
//!   [RFC 8949 section 3.4.4](https://www.rfc-editor.org/rfc/rfc8949#section-3.4.4)).
//!   `bver` may only be an unsigned integer. Text and byte strings must have a
//!   definite length (their length is written before them).
//! - A SenML pack is a definite-length array. A SenSML stream may also be an
//!   indefinite-length one, which ends with a "break" byte (0xff,
//!   [RFC 8949 section 3.2.1](https://www.rfc-editor.org/rfc/rfc8949#section-3.2.1)),
//!   and it should be.

use alloc::borrow::Cow;

use minicbor::Decoder;
use minicbor::data::{Tag, Type};
use minicbor::decode::Error as CborError;
use rsenml_core::codec::{FieldValue, RecordBuilder, number};
use rsenml_core::{
    Error, Format, Label, LabelType, Pack, Record, Result, ValidationOptions, Validator,
};

/// CBOR tag 4, decimal fraction `[exponent, mantissa]`, meaning
/// mantissa * 10^exponent
/// ([RFC 8949 section 3.4.4](https://www.rfc-editor.org/rfc/rfc8949#section-3.4.4)).
const DECIMAL_FRACTION: u64 = 4;

/// Reads a complete SenML CBOR pack (a definite-length array) and validates it.
/// Text and binary fields borrow from `input`.
pub fn decode(input: &[u8]) -> Result<Pack<'_>> {
    let pack = decode_unchecked(input)?;
    pack.validate()?;
    Ok(pack)
}

/// Reads a complete SenML CBOR pack, checking only the CBOR form (see the module
/// documentation). Use [`Pack::validate`] for the rules of section 4.
pub fn decode_unchecked(input: &[u8]) -> Result<Pack<'_>> {
    read_all(input, false)
}

/// Reads a complete SenSML CBOR stream, whose array may have an indefinite length
/// ([section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)), and validates
/// it.
pub fn decode_sensml(input: &[u8]) -> Result<Pack<'_>> {
    let pack = read_all(input, true)?;
    pack.validate()?;
    Ok(pack)
}

fn read_all(input: &[u8], streaming: bool) -> Result<Pack<'_>> {
    let mut reader = RecordReader::with_mode(input, streaming);
    let mut pack = Pack::new();
    while let Some(record) = reader.next_record()? {
        pack.push(record);
    }
    reader.finish()?;
    Ok(pack)
}

/// Where the reader is in the outer array: before it (`Start`), inside it
/// (`Items`), or after it (`Done`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Start,
    /// Inside the array. `Some(n)`: n records left in a definite-length array.
    /// `None`: an indefinite-length array, which ends at a break byte.
    Items(Option<u64>),
    Done,
}

#[derive(Debug, Clone)]
struct Cursor {
    pos: usize,
    state: State,
    index: usize,
    /// Accept an indefinite-length array (SenSML).
    streaming: bool,
}

// Returned once per record and unpacked right away. Boxing the record would add an
// allocation.
#[allow(clippy::large_enum_variant)]
enum Step<'a> {
    Record(Record<'a>),
    End,
}

impl Cursor {
    fn new(streaming: bool) -> Self {
        Self {
            pos: 0,
            state: State::Start,
            index: 0,
            streaming,
        }
    }

    /// One record (or the end of the array). Errors leave the cursor where it was,
    /// so a caller that gets [`Error::Incomplete`] can add bytes and try again.
    /// (Only `self.pos` and `self.state` move, and only after a full step.)
    fn step<'a>(&mut self, input: &'a [u8]) -> Result<Step<'a>> {
        let mut d = Decoder::new(input);
        d.set_position(self.pos);
        loop {
            match self.state {
                State::Done => return Ok(Step::End),
                State::Start => {
                    let at = d.position();
                    let len = match d.datatype().map_err(|e| cbor(e, at))? {
                        Type::Array => d.array().map_err(|e| cbor(e, at))?,
                        Type::ArrayIndef if self.streaming => d.array().map_err(|e| cbor(e, at))?,
                        Type::ArrayIndef => {
                            return Err(syntax(
                                at,
                                "a SenML pack must be a definite-length array (SenSML may be indefinite)",
                            ));
                        }
                        _ => return Err(syntax(at, "a SenML pack is a CBOR array")),
                    };
                    self.pos = d.position();
                    self.state = State::Items(len);
                }
                State::Items(Some(0)) => {
                    self.state = State::Done;
                    return Ok(Step::End);
                }
                State::Items(left) => {
                    let at = d.position();
                    // In an indefinite-length array, a break byte ends the array.
                    if left.is_none() && d.datatype().map_err(|e| cbor(e, at))? == Type::Break {
                        d.set_position(at + 1);
                        self.pos = at + 1;
                        self.state = State::Done;
                        return Ok(Step::End);
                    }
                    let record = record(&mut d, self.index)?;
                    self.pos = d.position();
                    self.index += 1;
                    self.state = State::Items(left.map(|n| n - 1));
                    return Ok(Step::Record(record));
                }
            }
        }
    }

    fn finish(&self, input: &[u8]) -> Result<()> {
        match self.state {
            State::Done if self.pos == input.len() => Ok(()),
            State::Done => Err(syntax(self.pos, "data after the end of the pack")),
            _ => Err(Error::Incomplete {
                format: Format::Cbor,
            }),
        }
    }
}

/// Reads the records of a complete SenML (or SenSML) CBOR pack one at a time.
#[derive(Debug, Clone)]
pub struct RecordReader<'a> {
    input: &'a [u8],
    cursor: Cursor,
}

impl<'a> RecordReader<'a> {
    /// A reader for a SenML pack (definite-length array).
    pub fn new(input: &'a [u8]) -> Self {
        Self::with_mode(input, false)
    }

    /// A reader for a SenSML stream (definite- or indefinite-length array).
    pub fn sensml(input: &'a [u8]) -> Self {
        Self::with_mode(input, true)
    }

    fn with_mode(input: &'a [u8], streaming: bool) -> Self {
        Self {
            input,
            cursor: Cursor::new(streaming),
        }
    }

    /// The next record, or `None` at the end of the array.
    pub fn next_record(&mut self) -> Result<Option<Record<'a>>> {
        match self.cursor.step(self.input)? {
            Step::Record(r) => Ok(Some(r)),
            Step::End => Ok(None),
        }
    }

    /// Byte offset after the last record read.
    pub fn position(&self) -> usize {
        self.cursor.pos
    }

    /// Checks that the array ended and nothing follows it.
    pub fn finish(&self) -> Result<()> {
        self.cursor.finish(self.input)
    }
}

impl<'a> Iterator for RecordReader<'a> {
    type Item = Result<Record<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.next_record().transpose();
        if matches!(item, Some(Err(_))) {
            self.cursor.state = State::Done;
            self.cursor.pos = self.input.len();
        }
        item
    }
}

/// Decodes a SenSML CBOR stream
/// ([section 4.8](https://www.rfc-editor.org/rfc/rfc8428#section-4.8)) as its bytes
/// arrive. Each record is ready once all of its bytes are in.
#[derive(Debug, Clone)]
pub struct StreamDecoder {
    buf: alloc::vec::Vec<u8>,
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
            buf: alloc::vec::Vec::new(),
            cursor: Cursor::new(true),
            validator: Validator::new(options),
        }
    }

    /// Appends received bytes.
    pub fn push(&mut self, bytes: &[u8]) {
        // Drop the bytes already read once they fill more than half of the buffer
        // (and more than 4096 bytes).
        if self.cursor.pos > 4096 && self.cursor.pos * 2 > self.buf.len() {
            self.buf.drain(..self.cursor.pos);
            self.cursor.pos = 0;
        }
        self.buf.extend_from_slice(bytes);
    }

    /// The next complete record (validated). `None` when more bytes are needed or
    /// the stream has ended ([`is_finished`](Self::is_finished)). The record is
    /// copied out of the buffer, because the buffer changes on the next push.
    pub fn next_record(&mut self) -> Result<Option<Record<'static>>> {
        match self.cursor.step(&self.buf) {
            Ok(Step::Record(r)) => {
                let r = r.into_owned();
                self.validator.check(&r)?;
                Ok(Some(r))
            }
            Ok(Step::End) | Err(Error::Incomplete { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The end of the array (or its break code) has been read.
    pub fn is_finished(&self) -> bool {
        self.cursor.state == State::Done
    }

    /// Ends the stream. It must be closed, hold at least one record, and have
    /// nothing after it.
    pub fn finish(&self) -> Result<()> {
        self.cursor.finish(&self.buf)?;
        self.validator.finish()
    }
}

fn syntax(offset: usize, reason: &'static str) -> Error {
    Error::Syntax {
        format: Format::Cbor,
        offset,
        reason: reason.into(),
    }
}

fn cbor(e: CborError, at: usize) -> Error {
    if e.is_end_of_input() {
        Error::Incomplete {
            format: Format::Cbor,
        }
    } else {
        Error::Syntax {
            format: Format::Cbor,
            offset: e.position().unwrap_or(at),
            // Fixed reasons: formatting minicbor's error would pull in core::fmt.
            reason: if e.is_type_mismatch() {
                "unexpected CBOR data item"
            } else {
                "malformed CBOR"
            }
            .into(),
        }
    }
}

// One record: a CBOR map, of definite length (`left` counts down) or indefinite
// length (ends at a break byte).
fn record<'a>(d: &mut Decoder<'a>, index: usize) -> Result<Record<'a>> {
    let at = d.position();
    let len = match d.datatype().map_err(|e| cbor(e, at))? {
        Type::Map | Type::MapIndef => d.map().map_err(|e| cbor(e, at))?,
        _ => return Err(syntax(at, "a SenML record is a CBOR map")),
    };
    let mut b = RecordBuilder::new(index);
    let mut left = len;
    loop {
        let at = d.position();
        match left {
            Some(0) => break,
            None if d.datatype().map_err(|e| cbor(e, at))? == Type::Break => {
                d.set_position(at + 1);
                break;
            }
            _ => {}
        }
        entry(d, &mut b, index)?;
        left = left.map(|n| n - 1);
    }
    Ok(b.finish())
}

fn entry<'a>(d: &mut Decoder<'a>, b: &mut RecordBuilder<'a>, index: usize) -> Result<()> {
    let at = d.position();
    match d.datatype().map_err(|e| cbor(e, at))? {
        Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::I8
        | Type::I16
        | Type::I32
        | Type::I64
        | Type::Int => {
            let key = d.int().map_err(|e| cbor(e, at))?;
            match i64::try_from(key).ok().and_then(Label::from_cbor) {
                Some(label) => {
                    let value = field(d, label, index)?;
                    b.set(label, value)
                }
                // Table 4 is complete (section 6 says no more integer keys will be
                // added), so other integer keys carry nothing we know.
                None => skip(d),
            }
        }
        Type::String => {
            let key = d.str().map_err(|e| cbor(e, at))?;
            if Label::from_text(key).is_some() {
                // RFC 8428 4.4: "MUST ignore fields they don't recognize". In CBOR, a
                // registered label is its Table 4 integer (section 6), and labels
                // added later are text keys (12.2; RFC 9193 section 8).
                // https://www.rfc-editor.org/rfc/rfc8428#section-4.4
                // https://www.rfc-editor.org/rfc/rfc8428#section-12.2
                // https://www.rfc-editor.org/rfc/rfc9193#section-8
                return skip(d);
            }
            extension(d, b, Cow::Borrowed(key))
        }
        Type::StringIndef => Err(syntax(at, "map keys must be definite-length strings")),
        _ => Err(syntax(at, "map keys must be integers or text strings")),
    }
}

// The value of a registered label, which must have the type the label needs.
fn field<'a>(d: &mut Decoder<'a>, label: Label, index: usize) -> Result<FieldValue<'a>> {
    let at = d.position();
    let ty = d.datatype().map_err(|e| cbor(e, at))?;
    let wrong = Error::WrongType {
        record: index,
        label: label.text(),
    };
    match label.value_type() {
        LabelType::String => match ty {
            Type::String => Ok(FieldValue::String(Cow::Borrowed(
                d.str().map_err(|e| cbor(e, at))?,
            ))),
            Type::StringIndef => Err(syntax(at, "text must have a definite length")),
            _ => Err(wrong),
        },
        LabelType::Number => numeric(d, ty)?.ok_or(wrong),
        LabelType::Version => match ty {
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => {
                Ok(FieldValue::Unsigned(d.u64().map_err(|e| cbor(e, at))?))
            }
            _ => Err(wrong),
        },
        LabelType::Boolean => match ty {
            Type::Bool => Ok(FieldValue::Bool(d.bool().map_err(|e| cbor(e, at))?)),
            _ => Err(wrong),
        },
        LabelType::Data => match ty {
            Type::Bytes => Ok(FieldValue::Data(Cow::Borrowed(
                d.bytes().map_err(|e| cbor(e, at))?,
            ))),
            Type::BytesIndef => Err(syntax(at, "Data Value must have a definite length")),
            _ => Err(wrong),
        },
    }
}

/// A number of any CBOR form SenML allows, or `None` (nothing read) when the item
/// is not a number. SenML allows integers, floats and decimal fractions
/// ([RFC 8428 section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)).
fn numeric(d: &mut Decoder<'_>, ty: Type) -> Result<Option<FieldValue<'static>>> {
    let at = d.position();
    let map = |e| cbor(e, at);
    Ok(Some(match ty {
        Type::U8 | Type::U16 | Type::U32 | Type::U64 => FieldValue::Unsigned(d.u64().map_err(map)?),
        Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::Int => {
            // CBOR negative integers go down to -2^64, below i64, so go through i128.
            FieldValue::Number(i128::from(d.int().map_err(map)?) as f64)
        }
        // Half-precision float: the initial byte 0xf9, then 2 bytes of IEEE half
        // bits, big-endian
        // (https://www.rfc-editor.org/rfc/rfc8949#section-3.3). It is read by hand
        // and turned into a double exactly.
        Type::F16 => {
            let bytes = d.input().get(at + 1..at + 3).ok_or(Error::Incomplete {
                format: Format::Cbor,
            })?;
            let bits = u16::from_be_bytes([bytes[0], bytes[1]]);
            d.set_position(at + 3);
            FieldValue::Number(number::f16_to_f64(bits))
        }
        Type::F32 => FieldValue::Number(f64::from(d.f32().map_err(map)?)),
        Type::F64 => FieldValue::Number(d.f64().map_err(map)?),
        Type::Tag => {
            // Look at the tag without reading it. Any tag other than 4 is not a
            // number.
            let mut probe = d.probe();
            if probe.tag().map_err(map)? != Tag::new(DECIMAL_FRACTION) {
                return Ok(None);
            }
            d.tag().map_err(map)?;
            FieldValue::Number(decimal_fraction(d)?)
        }
        _ => return Ok(None),
    }))
}

/// The body of a tag 4 decimal fraction: `[exponent, mantissa]`, both integers
/// ([RFC 8949 section 3.4.4](https://www.rfc-editor.org/rfc/rfc8949#section-3.4.4)).
/// The value is rounded once to the nearest double.
fn decimal_fraction(d: &mut Decoder<'_>) -> Result<f64> {
    let at = d.position();
    let map = |e| cbor(e, at);
    if d.array().map_err(map)? != Some(2) {
        return Err(syntax(at, "a decimal fraction is a 2-element array"));
    }
    let exponent = i64::try_from(d.int().map_err(map)?)
        .map_err(|_| syntax(at, "decimal fraction exponent out of range"))?;
    let mantissa = i128::from(d.int().map_err(map)?);
    Ok(number::decimal_to_f64(mantissa, exponent))
}

// The value of an unknown text key. Scalars are kept; arrays, maps, other tags and
// the like are skipped (after the `_` check).
fn extension<'a>(
    d: &mut Decoder<'a>,
    b: &mut RecordBuilder<'a>,
    label: Cow<'a, str>,
) -> Result<()> {
    let at = d.position();
    let ty = d.datatype().map_err(|e| cbor(e, at))?;
    let value = match ty {
        Type::String => FieldValue::String(Cow::Borrowed(d.str().map_err(|e| cbor(e, at))?)),
        Type::Bytes => FieldValue::Data(Cow::Borrowed(d.bytes().map_err(|e| cbor(e, at))?)),
        Type::Bool => FieldValue::Bool(d.bool().map_err(|e| cbor(e, at))?),
        _ => match numeric(d, ty)? {
            Some(v) => v,
            None => {
                b.skipped_extension(&label)?;
                return skip(d);
            }
        },
    };
    b.extension(label, value)
}

fn skip(d: &mut Decoder<'_>) -> Result<()> {
    let at = d.position();
    d.skip().map_err(|e| cbor(e, at))
}
