//! Writing SenML EXI
//! ([RFC 8428 section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8)).
//!
//! The writer drives rexi's encoder with the ids that rexi-schema-gen made for the
//! schema's names (see `attribute_qname`), not with name strings. In a
//! schema-informed grammar each element and attribute already has a fixed place,
//! so the encoder can go straight to the right production (grammar rule) and write
//! its event code ([EXI 6.1](https://www.w3.org/TR/exi/#eventCodes)).

use alloc::string::ToString;
use alloc::vec::Vec;

use rexi_core::IoErrorKind;
use rexi_core::io::{ByteSink, SliceSink};
use rexi_core::{Encoder, Event, Float, HeaderConfig, Int, QNameId, Value as ExiValue};
use rsenml_core::codec::number;
use rsenml_core::{Error, Format, Label, Pack, Record, Result, Value, base64url};

use crate::{Alignment, exi_error, exi_options, tables};

/// What to do with extension fields, which strict EXI cannot carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Extensions {
    /// Leave them out
    /// ([RFC 8428 section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8):
    /// "any extensions will be lost").
    #[default]
    Drop,
    /// Refuse to encode a pack that has any.
    Error,
}

/// How to write the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct EncodeOptions {
    /// Bit-packed (default) or byte-aligned body.
    pub alignment: Alignment,
    /// Write the `$EXI` cookie ([EXI 5.1](https://www.w3.org/TR/exi/#EXICookie)).
    /// RFC 8428 section 8 says SHOULD NOT over CoAP or HTTP.
    pub cookie: bool,
    /// Extensions are dropped (default) or refused.
    pub extensions: Extensions,
}

impl EncodeOptions {
    /// Bit-packed, no cookie, extensions dropped (the default).
    pub const fn new() -> Self {
        Self {
            alignment: Alignment::BitPacked,
            cookie: false,
            extensions: Extensions::Drop,
        }
    }

    /// Sets [`EncodeOptions::alignment`].
    pub const fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Sets [`EncodeOptions::cookie`].
    pub const fn with_cookie(mut self, cookie: bool) -> Self {
        self.cookie = cookie;
        self
    }

    /// Sets [`EncodeOptions::extensions`].
    pub const fn with_extensions(mut self, extensions: Extensions) -> Self {
        self.extensions = extensions;
        self
    }
}

/// Validates a pack and writes it bit-packed, with the options and schemaId "a" in
/// the header.
pub fn encode(pack: &Pack<'_>) -> Result<Vec<u8>> {
    encode_with(pack, &EncodeOptions::default())
}

/// [`encode`] with [`EncodeOptions`].
pub fn encode_with(pack: &Pack<'_>, options: &EncodeOptions) -> Result<Vec<u8>> {
    pack.validate()?;
    encode_unchecked_with(pack, options)
}

/// Writes a pack as it is, without validating it first (for packs already
/// validated, for example read by a decoder or built by trusted code). Values EXI
/// cannot carry are still refused.
pub fn encode_unchecked(pack: &Pack<'_>) -> Result<Vec<u8>> {
    encode_unchecked_with(pack, &EncodeOptions::default())
}

/// [`encode_unchecked`] with [`EncodeOptions`].
pub fn encode_unchecked_with(pack: &Pack<'_>, options: &EncodeOptions) -> Result<Vec<u8>> {
    let mut w = StreamEncoder::new(Vec::new(), options)?;
    for record in pack {
        w.record(record)?;
    }
    w.finish()
}

/// Validates a pack and writes it into `buf`. Returns the length, or
/// [`Error::BufferFull`].
pub fn encode_into(pack: &Pack<'_>, buf: &mut [u8], options: &EncodeOptions) -> Result<usize> {
    pack.validate()?;
    let mut w = StreamEncoder::new(SliceBuf::new(buf), options)?;
    for record in pack {
        w.record(record)?;
    }
    Ok(w.finish()?.len())
}

/// Where a [`StreamEncoder`] writes: a growable `Vec<u8>` or a fixed [`SliceBuf`].
/// Sealed (no other crate can add a type to it), because the encoder's byte
/// interface is rexi's, which is not part of rsenml's API.
pub trait Sink: sealed::Sink {}

mod sealed {
    /// rexi's byte sink, kept out of the public API.
    pub trait Sink: rexi_core::io::ByteSink {}
}

impl sealed::Sink for Vec<u8> {}
impl Sink for Vec<u8> {}
impl sealed::Sink for SliceBuf<'_> {}
impl Sink for SliceBuf<'_> {}

/// A buffer the caller gives to [`StreamEncoder`], for targets without a heap to
/// spare. Writing past its end fails with [`Error::BufferFull`].
#[derive(Debug)]
pub struct SliceBuf<'a>(SliceSink<'a>);

impl<'a> SliceBuf<'a> {
    /// A buffer to write into, from its start.
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self(SliceSink::new(buf))
    }

    /// Number of bytes written so far.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Nothing written yet.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The bytes written so far.
    pub fn written(&self) -> &[u8] {
        self.0.written()
    }
}

impl ByteSink for SliceBuf<'_> {
    fn put_bytes(&mut self, bytes: &[u8]) -> core::result::Result<(), IoErrorKind> {
        self.0.put_bytes(bytes)
    }
}

/// Writes a SenML (or SenSML) EXI stream one record at a time.
/// [`finish`](Self::finish) closes the `sensml` element. It does not validate.
pub struct StreamEncoder<W: Sink> {
    enc: Encoder<W, &'static rexi_core::GrammarSet<'static>>,
    extensions: Extensions,
    index: usize,
}

impl<W: Sink> core::fmt::Debug for StreamEncoder<W> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StreamEncoder")
            .field("extensions", &self.extensions)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

/// Registered labels in the schema's attribute order: sorted by name, as the
/// complex type grammar sorts attribute uses
/// ([EXI 8.5.4.1.3.2](https://www.w3.org/TR/exi/#complexTypeGrammars)). All SenML
/// attributes are optional, so after attribute k the grammar only offers the
/// attributes after k, or the end of the element. The attributes must be written in
/// this order.
const ATTRIBUTE_ORDER: [Label; 15] = [
    Label::BaseName,
    Label::BaseSum,
    Label::BaseTime,
    Label::BaseUnit,
    Label::BaseValue,
    Label::BaseVersion,
    Label::Name,
    Label::Sum,
    Label::Time,
    Label::Unit,
    Label::UpdateTime,
    Label::Value,
    Label::BoolValue,
    Label::DataValue,
    Label::StringValue,
];

impl<W: Sink> StreamEncoder<W> {
    /// Writes the header ([EXI 5](https://www.w3.org/TR/exi/#header)), with the
    /// options always included, and opens the `sensml` element.
    pub fn new(sink: W, options: &EncodeOptions) -> Result<Self> {
        let header = HeaderConfig::new()
            .with_cookie(options.cookie)
            .with_include_options(true);
        let enc = Encoder::new(
            sink,
            &tables::GRAMMARS,
            exi_options(options.alignment),
            header,
        )
        .map_err(|e| exi_error(e, 0))?;
        let mut w = Self {
            enc,
            extensions: options.extensions,
            index: 0,
        };
        w.push(&Event::StartDocument)?;
        w.checked(|enc| enc.start_element_id(tables::LN_SENSML))?;
        Ok(w)
    }

    /// Appends one `senml` element.
    pub fn record(&mut self, r: &Record<'_>) -> Result<()> {
        let index = self.index;
        self.index += 1;
        if !r.extensions.is_empty() && self.extensions == Extensions::Error {
            return Err(Error::Unrepresentable {
                format: Format::Exi,
                reason: "strict EXI cannot carry extension labels".into(),
            });
        }
        // Numbers are checked before anything is written, so a bad record leaves no
        // half-written element. The Data Value's base64url text is made first, so
        // that its event can borrow it (the schema types `vd` as xs:string).
        for label in ATTRIBUTE_ORDER {
            if let Some(v) = number_of(r, label)
                && !v.is_finite()
            {
                return Err(Error::NonFinite {
                    record: index,
                    label: label.text().to_string(),
                });
            }
        }
        let data = match &r.value {
            Some(Value::Data(d)) => Some(base64url::encode(d)),
            _ => None,
        };
        self.checked(|enc| enc.start_element_id(tables::LN_SENML))?;
        for label in ATTRIBUTE_ORDER {
            let value = match label {
                Label::BaseName => r.base_name.as_deref().map(ExiValue::String),
                Label::BaseUnit => r.base_unit.as_deref().map(ExiValue::String),
                Label::Name => r.name.as_deref().map(ExiValue::String),
                Label::Unit => r.unit.as_deref().map(ExiValue::String),
                Label::StringValue => match &r.value {
                    Some(Value::String(s)) => Some(ExiValue::String(s)),
                    _ => None,
                },
                Label::BoolValue => match &r.value {
                    Some(Value::Bool(b)) => Some(ExiValue::Boolean(*b)),
                    _ => None,
                },
                Label::BaseVersion => match r.base_version {
                    // xs:int (https://www.w3.org/TR/xmlschema-2/#int)
                    Some(v @ 0..=2_147_483_647) => Some(ExiValue::Integer(Int::Small(v as i64))),
                    Some(_) => {
                        return Err(Error::Unrepresentable {
                            format: Format::Exi,
                            reason: "bver above the xs:int range".into(),
                        });
                    }
                    None => None,
                },
                Label::DataValue => data.as_deref().map(ExiValue::String),
                // Doubles go to rexi as EXI Floats (a decimal mantissa and exponent,
                // https://www.w3.org/TR/exi/#encodingFloat): the shortest decimal
                // digits, with trailing zeros moved into the exponent. That is how
                // rexi and EXIficient put the same XML attribute into standard form
                // (EXI C14N, Canonical EXI). So the bytes are the same as for the text
                // form, without the round trip through text. A double's decimal
                // exponent is at most a few hundred, so it fits an i16.
                _ => number_of(r, label).map(|v| {
                    let (mantissa, exponent) = number::decimal_parts(v);
                    ExiValue::Float(Float {
                        mantissa,
                        exponent: exponent as i16,
                    })
                }),
            };
            if let Some(value) = value {
                let qname = attribute_qname(label);
                self.checked(|enc| enc.attribute_id(qname, &value))?;
            }
        }
        self.push(&Event::EndElement)
    }

    /// Closes `sensml` and the document. Returns the sink.
    pub fn finish(mut self) -> Result<W> {
        self.push(&Event::EndElement)?;
        self.push(&Event::EndDocument)?;
        let bit = self.enc.bit_position();
        self.enc.finish().map_err(|e| exi_error(e, bit))
    }

    fn push(&mut self, ev: &Event<'_>) -> Result<()> {
        self.checked(|enc| enc.push(ev))
    }

    /// Runs one encoder call. On error, reports the bit position from before the
    /// call.
    fn checked(
        &mut self,
        f: impl FnOnce(
            &mut Encoder<W, &'static rexi_core::GrammarSet<'static>>,
        ) -> rexi_core::ExiResult<()>,
    ) -> Result<()> {
        let bit = self.enc.bit_position();
        f(&mut self.enc).map_err(|e| exi_error(e, bit))
    }
}

/// The numeric field of `r` that `label` names, if any.
fn number_of(r: &Record<'_>, label: Label) -> Option<f64> {
    match label {
        Label::BaseTime => r.base_time,
        Label::BaseValue => r.base_value,
        Label::BaseSum => r.base_sum,
        Label::Sum => r.sum,
        Label::Time => r.time,
        Label::UpdateTime => r.update_time,
        Label::Value => r.value.as_ref().and_then(Value::as_number),
        _ => None,
    }
}

/// The generated qname id of a label's attribute. A *qname* is a namespace URI plus
/// a local name. Its id is the pair of compact identifiers (small numbers) that the
/// string table gives the URI and the local name: here URI 0 (the empty
/// namespace) and the local name's place in the sorted list of schema names
/// ([EXI D.3](https://www.w3.org/TR/exi/#initialLocalNames)). It is worked out at
/// compile time, so the encoder does no name lookup.
fn attribute_qname(label: Label) -> QNameId {
    match label {
        Label::BaseName => tables::LN_BN,
        Label::BaseSum => tables::LN_BS,
        Label::BaseTime => tables::LN_BT,
        Label::BaseUnit => tables::LN_BU,
        Label::BaseValue => tables::LN_BV,
        Label::BaseVersion => tables::LN_BVER,
        Label::Name => tables::LN_N,
        Label::Sum => tables::LN_S,
        Label::Time => tables::LN_T,
        Label::Unit => tables::LN_U,
        Label::UpdateTime => tables::LN_UT,
        Label::Value => tables::LN_V,
        Label::BoolValue => tables::LN_VB,
        Label::DataValue => tables::LN_VD,
        Label::StringValue => tables::LN_VS,
    }
}
