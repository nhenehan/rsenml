//! Writing SenML CBOR
//! ([RFC 8428 section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)).
//!
//! Every number is written in the smallest form that reads back as the same
//! double. Section 6 only asks that it reads back as the same double; picking the
//! smallest such form is rsenml's choice. The form is an integer when it is a whole number,
//! otherwise a half, single or double float. For floats this is also CBOR's
//! "preferred serialization"
//! ([RFC 8949 section 4.1](https://www.rfc-editor.org/rfc/rfc8949#section-4.1)).
//! Records are definite-length maps, with the integer keys of Table 4 in
//! [`Label::WRITE_ORDER`], then extensions under text keys. A SenML pack is a
//! definite-length array; a SenSML stream is an indefinite-length one. This writes
//! the RFC's section 6 example byte for byte.

use alloc::string::String;
use alloc::vec::Vec;
use core::convert::Infallible;

use minicbor::Encoder;
use minicbor::encode::write::{Cursor, EndOfSlice};
use minicbor::encode::{Error as EncodeError, Write};
use rsenml_core::codec::number;
use rsenml_core::{Error, Format, Label, Pack, Record, Result, Value};

/// Writes a pack (definite-length array) after validating it.
pub fn encode(pack: &Pack<'_>) -> Result<Vec<u8>> {
    pack.validate()?;
    encode_unchecked(pack)
}

/// Writes a pack as it is, without validating (infinite and NaN numbers are still
/// refused).
pub fn encode_unchecked(pack: &Pack<'_>) -> Result<Vec<u8>> {
    let mut e = Encoder::new(Vec::new());
    e.array(pack.len() as u64).map_err(map_err)?;
    for (i, r) in pack.iter().enumerate() {
        record(&mut e, r, i)?;
    }
    Ok(e.into_writer())
}

/// Validates a pack and writes it as a SenSML stream: an indefinite-length array.
pub fn encode_sensml(pack: &Pack<'_>) -> Result<Vec<u8>> {
    pack.validate()?;
    let mut out = Vec::new();
    let mut s = StreamEncoder::new();
    for r in pack {
        s.record(r, &mut out)?;
    }
    s.finish(&mut out);
    Ok(out)
}

/// Validates a pack and writes it into `buf`. Returns the length, or
/// [`Error::BufferFull`].
pub fn encode_into(pack: &Pack<'_>, buf: &mut [u8]) -> Result<usize> {
    pack.validate()?;
    let mut e = Encoder::new(Cursor::new(buf));
    e.array(pack.len() as u64).map_err(map_err)?;
    for (i, r) in pack.iter().enumerate() {
        record(&mut e, r, i)?;
    }
    Ok(e.into_writer().position())
}

/// Writes one record map.
pub fn encode_record(r: &Record<'_>, index: usize, out: &mut Vec<u8>) -> Result<()> {
    let mut e = Encoder::new(out);
    record(&mut e, r, index)
}

/// Writes a SenSML CBOR stream one piece at a time: the start of an
/// indefinite-length array (`0x9f`), one map per record, then the break byte
/// (`0xff`) that ends the array
/// ([RFC 8949 section 3.2.2](https://www.rfc-editor.org/rfc/rfc8949#section-3.2.2)).
/// It does not validate.
#[derive(Debug, Clone, Default)]
pub struct StreamEncoder {
    started: bool,
    records: usize,
}

impl StreamEncoder {
    /// An encoder that has written nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends the next record (with the array header `0x9f` before the first).
    pub fn record(&mut self, r: &Record<'_>, out: &mut Vec<u8>) -> Result<()> {
        if !self.started {
            out.push(0x9f);
            self.started = true;
        }
        encode_record(r, self.records, out)?;
        self.records += 1;
        Ok(())
    }

    /// Appends the break code `0xff` (and the header if no record was written).
    pub fn finish(self, out: &mut Vec<u8>) {
        if !self.started {
            out.push(0x9f);
        }
        out.push(0xff);
    }
}

/// Lets [`map_err`] tell a full output buffer from other writer errors.
trait SinkError: core::fmt::Debug {
    fn is_full(&self) -> bool;
}

impl SinkError for Infallible {
    fn is_full(&self) -> bool {
        false
    }
}

impl SinkError for EndOfSlice {
    fn is_full(&self) -> bool {
        true
    }
}

fn map_err<E: SinkError>(e: EncodeError<E>) -> Error {
    match e.as_write() {
        Some(w) if w.is_full() => Error::BufferFull,
        _ => Error::Unrepresentable {
            format: Format::Cbor,
            reason: "CBOR encoder error".into(),
        },
    }
}

// The number of map entries, needed up front for a definite-length map. It must
// match what `record` writes field by field.
fn field_count(r: &Record<'_>) -> u64 {
    let fixed = [
        r.base_name.is_some(),
        r.base_time.is_some(),
        r.base_unit.is_some(),
        r.base_value.is_some(),
        r.base_sum.is_some(),
        r.base_version.is_some(),
        r.name.is_some(),
        r.unit.is_some(),
        r.value.is_some(),
        r.sum.is_some(),
        r.time.is_some(),
        r.update_time.is_some(),
    ];
    (fixed.iter().filter(|&&b| b).count() + r.extensions.len()) as u64
}

fn record<W: Write>(e: &mut Encoder<W>, r: &Record<'_>, index: usize) -> Result<()>
where
    W::Error: SinkError,
{
    e.map(field_count(r)).map_err(map_err)?;
    for label in Label::WRITE_ORDER {
        let key = i64::from(label.cbor());
        let num = |e: &mut Encoder<W>, v: Option<f64>| -> Result<()> {
            if let Some(v) = v {
                e.i64(key).map_err(map_err)?;
                write_number(e, v, label.text(), index)?;
            }
            Ok(())
        };
        let text = |e: &mut Encoder<W>, v: Option<&str>| -> Result<()> {
            if let Some(v) = v {
                e.i64(key).map_err(map_err)?.str(v).map_err(map_err)?;
            }
            Ok(())
        };
        match label {
            Label::BaseName => text(e, r.base_name.as_deref())?,
            Label::BaseUnit => text(e, r.base_unit.as_deref())?,
            Label::Name => text(e, r.name.as_deref())?,
            Label::Unit => text(e, r.unit.as_deref())?,
            Label::BaseTime => num(e, r.base_time)?,
            Label::BaseValue => num(e, r.base_value)?,
            Label::BaseSum => num(e, r.base_sum)?,
            Label::Time => num(e, r.time)?,
            Label::UpdateTime => num(e, r.update_time)?,
            Label::Sum => num(e, r.sum)?,
            Label::BaseVersion => {
                if let Some(v) = r.base_version {
                    e.i64(key).map_err(map_err)?.u64(v).map_err(map_err)?;
                }
            }
            // The record has at most one value. Write it at the place of its own
            // label in the order.
            Label::Value | Label::StringValue | Label::BoolValue | Label::DataValue => {
                if let Some(v) = r.value.as_ref().filter(|v| v.label() == label) {
                    e.i64(key).map_err(map_err)?;
                    write_value(e, v, label.text(), index)?;
                }
            }
        }
    }
    for ext in &r.extensions {
        e.str(&ext.label).map_err(map_err)?;
        write_value(e, &ext.value, &ext.label, index)?;
    }
    Ok(())
}

fn write_value<W: Write>(e: &mut Encoder<W>, v: &Value<'_>, label: &str, index: usize) -> Result<()>
where
    W::Error: SinkError,
{
    match v {
        Value::Number(v) => write_number(e, *v, label, index),
        Value::String(s) => e.str(s).map(|_| ()).map_err(map_err),
        Value::Bool(b) => e.bool(*b).map(|_| ()).map_err(map_err),
        Value::Data(d) => e.bytes(d).map(|_| ()).map_err(map_err),
    }
}

// The smallest exact form, tried in order: integer, half, single, double float.
// `-0.0` is not an integer (it would lose its sign), so it becomes a half float.
fn write_number<W: Write>(e: &mut Encoder<W>, v: f64, label: &str, index: usize) -> Result<()>
where
    W::Error: SinkError,
{
    if !v.is_finite() {
        return Err(Error::NonFinite {
            record: index,
            label: String::from(label),
        });
    }
    let r = if let Some(i) = number::as_exact_i64(v) {
        e.i64(i).map(|_| ())
    } else if let Some(bits) = number::to_f16_exact(v) {
        // minicbor writes half floats only with its `half` feature, and the bytes are
        // simple: 0xf9 (major type 7, additional info 25), then the 16 bits,
        // big-endian (https://www.rfc-editor.org/rfc/rfc8949#section-3.3).
        let [hi, lo] = bits.to_be_bytes();
        return e
            .writer_mut()
            .write_all(&[0xf9, hi, lo])
            .map_err(|w| map_err(EncodeError::write(w)));
    } else if let Some(f) = number::to_f32_exact(v) {
        e.f32(f).map(|_| ())
    } else {
        e.f64(v).map(|_| ())
    };
    r.map_err(map_err)
}
