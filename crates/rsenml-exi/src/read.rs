//! Reading SenML EXI
//! ([RFC 8428 section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8)).
//!
//! The header must carry EXI options
//! ([EXI 5.4](https://www.w3.org/TR/exi/#options)) that name schemaId "a". A stream
//! without options can be read when the caller gives them "out of band", that is,
//! outside the stream ([`DecodeOptions::out_of_band`]).

use alloc::borrow::Cow;
use alloc::string::{String, ToString};

use rexi_core::{Decoder, Event, Float, GrammarSet, Int, SchemaId, Value as ExiValue};
use rsenml_core::codec::{FieldValue, RecordBuilder, number};
use rsenml_core::{
    EXI_SCHEMA_ID, Error, Format, Label, LabelType, Pack, Record, Result, XML_NAMESPACE, base64url,
};

use crate::{Alignment, exi_error, exi_options, tables};

/// How to read a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct DecodeOptions {
    /// For streams whose header has no options (this does not follow RFC 8428, but
    /// it is smaller): read them as strict, schemaId "a", with this alignment.
    pub out_of_band: Option<Alignment>,
}

impl DecodeOptions {
    /// Options must be in the header (the default).
    pub const fn new() -> Self {
        Self { out_of_band: None }
    }

    /// Accept streams without options in the header, reading them as strict,
    /// schemaId "a", with this alignment ([`DecodeOptions::out_of_band`]).
    pub const fn with_out_of_band(mut self, alignment: Option<Alignment>) -> Self {
        self.out_of_band = alignment;
        self
    }
}

/// Reads a SenML EXI stream and validates the pack
/// ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
pub fn decode(input: &[u8]) -> Result<Pack<'static>> {
    decode_with(input, &DecodeOptions::default())
}

/// [`decode`] with [`DecodeOptions`] (for example out-of-band options).
pub fn decode_with(input: &[u8], options: &DecodeOptions) -> Result<Pack<'static>> {
    let pack = read_all(input, options)?;
    pack.validate()?;
    Ok(pack)
}

/// Reads a SenML EXI stream, checking only the EXI form (not the rules of
/// section 4).
pub fn decode_unchecked(input: &[u8]) -> Result<Pack<'static>> {
    read_all(input, &DecodeOptions::default())
}

fn read_all(input: &[u8], options: &DecodeOptions) -> Result<Pack<'static>> {
    let mut reader = RecordReader::new(input, options)?;
    let mut pack = Pack::new();
    while let Some(record) = reader.next_record()? {
        pack.push(record);
    }
    reader.finish()?;
    Ok(pack)
}

type Tables = &'static GrammarSet<'static>;

/// `Records` while inside `sensml`, `Done` after its end (or after an error).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Records,
    Done,
}

/// Reads the records of a SenML EXI stream one at a time.
pub struct RecordReader<'a> {
    dec: Decoder<&'a [u8], Tables>,
    state: State,
    index: usize,
}

impl core::fmt::Debug for RecordReader<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RecordReader")
            .field("state", &self.state)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

/// Whether the header says options follow
/// ([EXI 5](https://www.w3.org/TR/exi/#header)). After an optional `$EXI` cookie,
/// the first byte starts with the distinguishing bits `10`
/// ([EXI 5.2](https://www.w3.org/TR/exi/#DistinguishingBits)), then the presence
/// bit (0x20), which is 1 when the options are in the header.
fn header_has_options(input: &[u8]) -> Result<bool> {
    let rest = input.strip_prefix(b"$EXI").unwrap_or(input);
    match rest.first() {
        None => Err(Error::Incomplete {
            format: Format::Exi,
        }),
        Some(b) if b >> 6 == 0b10 => Ok(b & 0x20 != 0),
        Some(_) => Err(Error::Syntax {
            format: Format::Exi,
            offset: input.len() - rest.len(),
            reason: "not an EXI stream (distinguishing bits)".into(),
        }),
    }
}

impl<'a> RecordReader<'a> {
    /// Reads the header and the start of the `sensml` element. Options in the header
    /// must name schemaId "a". Without them, the caller's out-of-band options are
    /// used, or the stream is refused.
    pub fn new(input: &'a [u8], options: &DecodeOptions) -> Result<Self> {
        let in_header = header_has_options(input)?;
        let out_of_band = match (in_header, options.out_of_band) {
            (true, _) => None,
            (false, Some(alignment)) => Some(exi_options(alignment)),
            (false, None) => return Err(Error::SchemaId { found: None }),
        };
        let dec =
            Decoder::new(input, &tables::GRAMMARS, out_of_band).map_err(|e| exi_error(e, 0))?;
        if in_header {
            match &dec.options().schema_id {
                SchemaId::Id(id) if id == EXI_SCHEMA_ID => {}
                SchemaId::Id(id) => {
                    return Err(Error::SchemaId {
                        found: Some(id.clone()),
                    });
                }
                _ => return Err(Error::SchemaId { found: None }),
            }
        }
        let mut reader = Self {
            dec,
            state: State::Records,
            index: 0,
        };
        reader.expect(
            |e| matches!(e, Event::StartDocument),
            "missing start of document",
        )?;
        reader.expect(
            |e| matches!(e, Event::StartElement(q) if q.uri == XML_NAMESPACE && q.local == "sensml"),
            "the root element must be sensml",
        )?;
        Ok(reader)
    }

    /// The next record, or `None` at the end of `sensml`.
    pub fn next_record(&mut self) -> Result<Option<Record<'static>>> {
        if self.state == State::Done {
            return Ok(None);
        }
        let index = self.index;
        let bit = self.dec.bit_position();
        match self.dec.next_event().map_err(|e| exi_error(e, bit))? {
            Some(Event::StartElement(q)) if q.uri == XML_NAMESPACE && q.local == "senml" => {}
            Some(Event::EndElement) => {
                self.state = State::Done;
                self.expect(
                    |e| matches!(e, Event::EndDocument),
                    "missing end of document",
                )?;
                return Ok(None);
            }
            Some(_) => return Err(structure("only senml elements may appear inside sensml")),
            None => {
                return Err(Error::Incomplete {
                    format: Format::Exi,
                });
            }
        }
        self.index += 1;
        let mut b = RecordBuilder::new(index);
        loop {
            let bit = self.dec.bit_position();
            match self.dec.next_event().map_err(|e| exi_error(e, bit))? {
                // An attribute must be a SenML label in no namespace. The strict
                // grammar only allows schema attributes, but check anyway.
                Some(Event::Attribute(q, value)) => {
                    let label = Label::from_text(q.local)
                        .filter(|_| q.uri.is_empty())
                        .ok_or_else(|| structure("an attribute outside the SenML schema"))?;
                    let field = field(label, value, index)?;
                    b.set(label, field)?;
                }
                Some(Event::EndElement) => return Ok(Some(b.finish())),
                Some(_) => return Err(structure("a senml element has no content")),
                None => {
                    return Err(Error::Incomplete {
                        format: Format::Exi,
                    });
                }
            }
        }
    }

    /// Checks that the stream ended after `sensml`.
    pub fn finish(&mut self) -> Result<()> {
        if self.state != State::Done {
            return Err(Error::Incomplete {
                format: Format::Exi,
            });
        }
        let bit = self.dec.bit_position();
        match self.dec.next_event().map_err(|e| exi_error(e, bit))? {
            None => Ok(()),
            Some(_) => Err(structure("events after the end of the document")),
        }
    }

    fn expect(&mut self, ok: impl Fn(&Event<'_>) -> bool, reason: &'static str) -> Result<()> {
        let bit = self.dec.bit_position();
        match self.dec.next_event().map_err(|e| exi_error(e, bit))? {
            Some(e) if ok(&e) => Ok(()),
            Some(_) => Err(structure(reason)),
            None => Err(Error::Incomplete {
                format: Format::Exi,
            }),
        }
    }
}

impl Iterator for RecordReader<'_> {
    type Item = Result<Record<'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.next_record().transpose();
        if matches!(item, Some(Err(_))) {
            self.state = State::Done;
        }
        item
    }
}

fn structure(reason: &'static str) -> Error {
    Error::Structure {
        format: Format::Exi,
        reason,
    }
}

/// A typed EXI value as a SenML field. The schema gives each attribute its EXI
/// datatype ([EXI 7.1](https://www.w3.org/TR/exi/#encodingDatatypes)): strings for
/// `bn`, `bu`, `n`, `u`, `vs` and `vd` (base64url text), Float for numbers, Integer
/// for `bver` and Boolean for `vb`.
fn field(label: Label, value: ExiValue<'_>, index: usize) -> Result<FieldValue<'static>> {
    let wrong = || Error::WrongType {
        record: index,
        label: label.text(),
    };
    Ok(match (label.value_type(), value) {
        (LabelType::String, ExiValue::String(s)) => FieldValue::String(Cow::Owned(String::from(s))),
        (LabelType::Data, ExiValue::String(s)) => FieldValue::Data(Cow::Owned(
            base64url::decode(s).ok_or(Error::InvalidBase64 { record: index })?,
        )),
        (LabelType::Number, ExiValue::Float(f)) => {
            // An EXI Float with the special exponent -(2^14) is INF, -INF or NaN
            // (https://www.w3.org/TR/exi/#encodingFloat). These are not SenML
            // numbers.
            if f.exponent == Float::SPECIAL_EXPONENT {
                return Err(Error::NonFinite {
                    record: index,
                    label: label.text().to_string(),
                });
            }
            FieldValue::Number(number::from_decimal_parts(
                f.mantissa,
                i32::from(f.exponent),
            ))
        }
        (LabelType::Version, ExiValue::Integer(Int::Small(v))) => match u64::try_from(v) {
            Ok(v) => FieldValue::Unsigned(v),
            Err(_) => return Err(Error::InvalidVersion { record: index }),
        },
        (LabelType::Boolean, ExiValue::Boolean(b)) => FieldValue::Bool(b),
        _ => return Err(wrong()),
    })
}
