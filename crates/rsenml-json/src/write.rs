//! Writing SenML JSON
//! ([RFC 8428 section 5](https://www.rfc-editor.org/rfc/rfc8428#section-5)).
//!
//! Fields are written in [`Label::WRITE_ORDER`] (the order of the RFC's own
//! examples), with extensions after them. Numbers use the shortest text that reads
//! back as the same double (`23.1`, `20`, `1276020076.001`, `1e300`); see
//! [RFC 8259 section 6](https://www.rfc-editor.org/rfc/rfc8259#section-6) for JSON
//! numbers. Data Values are base64url without padding
//! ([RFC 4648 section 5](https://www.rfc-editor.org/rfc/rfc4648#section-5)). No
//! whitespace is written.

use alloc::string::String;
use alloc::vec::Vec;
use core::convert::Infallible;

use nanojson::{SerializeError, Serializer, SliceWriter, Write, WriteError};
use rsenml_core::codec::number;
use rsenml_core::{Error, Format, Label, Pack, Record, Result, Value, base64url};

/// Writes a pack after validating it
/// ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
pub fn encode(pack: &Pack<'_>) -> Result<Vec<u8>> {
    pack.validate()?;
    encode_unchecked(pack)
}

/// Writes a pack as it is, without validating. Infinite and NaN numbers are still
/// refused, because JSON has no way to write them.
pub fn encode_unchecked(pack: &Pack<'_>) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut w = Writer::new(VecSink(&mut out));
    w.pack(pack)?;
    Ok(out)
}

/// Validates a pack and writes it into `buf`, without allocating for the output.
/// Returns the length written, or [`Error::BufferFull`].
pub fn encode_into(pack: &Pack<'_>, buf: &mut [u8]) -> Result<usize> {
    pack.validate()?;
    let mut w = Writer::new(SliceWriter::new(buf));
    w.pack(pack)?;
    Ok(w.ser.into_writer().pos())
}

/// Writes one record object, for writing a SenSML stream
/// ([section 4.8](https://www.rfc-editor.org/rfc/rfc8428#section-4.8)) one piece at
/// a time. See [`StreamEncoder`].
pub fn encode_record(record: &Record<'_>, index: usize, out: &mut Vec<u8>) -> Result<()> {
    let mut w = Writer::new(VecSink(out));
    w.record(record, index)
}

/// Writes a SenSML JSON stream one piece at a time: `[`, then records separated by
/// `,`, then `]`. It does not validate. Run a [`rsenml_core::Validator`] next to it
/// if needed.
#[derive(Debug, Clone, Default)]
pub struct StreamEncoder {
    records: usize,
}

impl StreamEncoder {
    /// An encoder that has written nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends the next record (with the opening `[` before the first).
    pub fn record(&mut self, record: &Record<'_>, out: &mut Vec<u8>) -> Result<()> {
        out.push(if self.records == 0 { b'[' } else { b',' });
        encode_record(record, self.records, out)?;
        self.records += 1;
        Ok(())
    }

    /// Appends the closing `]` (or `[]` if no record was written).
    pub fn finish(self, out: &mut Vec<u8>) {
        if self.records == 0 {
            out.push(b'[');
        }
        out.push(b']');
    }
}

struct VecSink<'v>(&'v mut Vec<u8>);

impl Write for VecSink<'_> {
    type Error = Infallible;

    fn write_bytes(&mut self, b: &[u8]) -> core::result::Result<(), Infallible> {
        self.0.extend_from_slice(b);
        Ok(())
    }
}

/// Lets [`map_err`] tell a full output buffer from other sink errors.
trait SinkError: core::fmt::Debug {
    fn is_full(&self) -> bool;
}

impl SinkError for Infallible {
    fn is_full(&self) -> bool {
        false
    }
}

impl SinkError for WriteError {
    fn is_full(&self) -> bool {
        matches!(self, WriteError::BufferFull)
    }
}

fn map_err<E: SinkError>(e: SerializeError<E>) -> Error {
    match e {
        SerializeError::Write(w) if w.is_full() => Error::BufferFull,
        other => Error::Unrepresentable {
            format: Format::Json,
            reason: if matches!(other, SerializeError::Write(_)) {
                "writer failed"
            } else {
                "JSON serializer error"
            }
            .into(),
        },
    }
}

struct Writer<W: Write> {
    ser: Serializer<W>,
    num: String,
}

impl<W: Write> Writer<W>
where
    W::Error: SinkError,
{
    fn new(w: W) -> Self {
        Self {
            ser: Serializer::new(w),
            num: String::new(),
        }
    }

    fn pack(&mut self, pack: &Pack<'_>) -> Result<()> {
        self.ser.array_begin().map_err(map_err)?;
        for (i, record) in pack.iter().enumerate() {
            self.record(record, i)?;
        }
        self.ser.array_end().map_err(map_err)
    }

    // One record object. `index` is only used in error reports.
    fn record(&mut self, r: &Record<'_>, index: usize) -> Result<()> {
        self.ser.object_begin().map_err(map_err)?;
        for label in Label::WRITE_ORDER {
            let text = label.text();
            match label {
                Label::BaseName => self.string(text, r.base_name.as_deref())?,
                Label::BaseUnit => self.string(text, r.base_unit.as_deref())?,
                Label::Name => self.string(text, r.name.as_deref())?,
                Label::Unit => self.string(text, r.unit.as_deref())?,
                Label::BaseTime => self.number(text, r.base_time, index)?,
                Label::BaseValue => self.number(text, r.base_value, index)?,
                Label::BaseSum => self.number(text, r.base_sum, index)?,
                Label::Time => self.number(text, r.time, index)?,
                Label::UpdateTime => self.number(text, r.update_time, index)?,
                Label::Sum => self.number(text, r.sum, index)?,
                Label::BaseVersion => {
                    if let Some(v) = r.base_version {
                        self.ser.member(text).map_err(map_err)?;
                        self.ser.unsigned(v).map_err(map_err)?;
                    }
                }
                // The record has at most one value. Write it at the place of its own
                // label in the order.
                Label::Value | Label::StringValue | Label::BoolValue | Label::DataValue => {
                    if let Some(value) = r.value.as_ref().filter(|v| v.label() == label) {
                        self.ser.member(text).map_err(map_err)?;
                        self.value(value, text, index)?;
                    }
                }
            }
        }
        for ext in &r.extensions {
            self.ser.member(&ext.label).map_err(map_err)?;
            self.value(&ext.value, &ext.label, index)?;
        }
        self.ser.object_end().map_err(map_err)
    }

    fn string(&mut self, label: &str, value: Option<&str>) -> Result<()> {
        if let Some(s) = value {
            self.ser.member(label).map_err(map_err)?;
            self.ser.string(s).map_err(map_err)?;
        }
        Ok(())
    }

    fn number(&mut self, label: &str, value: Option<f64>, index: usize) -> Result<()> {
        if let Some(v) = value {
            self.ser.member(label).map_err(map_err)?;
            self.raw_number(v, label, index)?;
        }
        Ok(())
    }

    fn raw_number(&mut self, v: f64, label: &str, index: usize) -> Result<()> {
        if !v.is_finite() {
            return Err(Error::NonFinite {
                record: index,
                label: String::from(label),
            });
        }
        // The number text is built in a reused String and then written raw, so the
        // serializer does not format it a second time.
        self.num.clear();
        number::write_number(&mut self.num, v);
        self.ser.number_raw(self.num.as_bytes()).map_err(map_err)
    }

    fn value(&mut self, value: &Value<'_>, label: &str, index: usize) -> Result<()> {
        match value {
            Value::Number(v) => self.raw_number(*v, label, index),
            Value::String(s) => self.ser.string(s).map_err(map_err),
            Value::Bool(b) => self.ser.boolean(*b).map_err(map_err),
            Value::Data(d) => {
                self.num.clear();
                base64url::encode_into(d, &mut self.num);
                self.ser.string(&self.num).map_err(map_err)
            }
        }
    }
}
