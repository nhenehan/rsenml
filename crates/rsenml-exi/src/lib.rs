//! The EXI format of SenML
//! ([RFC 8428 section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8)): media
//! types `application/senml-exi` and `application/sensml-exi`
//! ([section 12.3](https://www.rfc-editor.org/rfc/rfc8428#section-12.3)).
//!
//! EXI (Efficient XML Interchange) is a binary form of XML. This crate uses
//! *strict schema-informed* EXI of the RFC's XSD (`spec/senml.xsd`). That means the
//! grammars (the state machines that say which events may come next, and the short
//! *event codes* that pick one) are built from the schema
//! ([EXI 8.5](https://www.w3.org/TR/exi/#informedGrammars)), and *strict* means
//! nothing outside the schema is allowed, so the codes stay short
//! ([EXI 5.4](https://www.w3.org/TR/exi/#options)). The grammars are compiled ahead
//! of time by rexi-schema-gen (`scripts/gen-tables.sh`), and rexi does the coding.
//! The header carries the EXI options with schemaId `"a"` (section 8 requires it).
//! By default there is no `$EXI` cookie
//! ([EXI 5.1](https://www.w3.org/TR/exi/#EXICookie)); section 8 advises against it
//! over CoAP and HTTP. The body is bit-packed by default and byte-aligned on
//! request. The RFC's two hex dumps are written byte for byte.
//!
//! ```
//! let pack = rsenml_json::decode(br#"[{"n":"urn:dev:ow:10e2073a01080063","u":"Cel","v":23.1}]"#)?;
//! let options = rsenml_exi::EncodeOptions::new().with_alignment(rsenml_exi::Alignment::ByteAligned);
//! let bytes = rsenml_exi::encode_with(&pack, &options)?;
//! assert_eq!(bytes.len(), 0x31); // RFC 8428 section 8, second example
//! assert_eq!(rsenml_exi::decode(&bytes)?, pack);
//! # Ok::<(), rsenml_core::Error>(())
//! ```
//!
//! Strict mode has no room for labels outside the schema. So extensions are dropped
//! (as section 8 says they will be) or refused ([`Extensions::Error`]).
//!
//! `no_std`; needs `alloc` (for rexi's string tables, the lists of strings already
//! seen, [EXI 7.3](https://www.w3.org/TR/exi/#stringTable)).

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

mod read;
mod write;

/// The RFC 8428 schema's grammar tables, as rexi-schema-gen writes them. Public for
/// tests that drive rexi directly. **Not covered by semver.**
#[doc(hidden)]
#[rustfmt::skip]
#[path = "generated/senml.rs"]
pub mod tables;

pub use read::{DecodeOptions, RecordReader, decode, decode_unchecked, decode_with};
pub use write::{
    EncodeOptions, Extensions, Sink, SliceBuf, StreamEncoder, encode, encode_into,
    encode_unchecked, encode_unchecked_with, encode_with,
};

/// EXI alignment of the body (the header is always bit-packed). This is the
/// `alignment` option of the EXI header
/// ([EXI 5.4](https://www.w3.org/TR/exi/#options)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    /// Event codes and values packed bit by bit: the smallest form (the default).
    BitPacked,
    /// Every event code and value starts on a byte boundary. This is simpler for a
    /// sensor to write by hand (the second example of
    /// [section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8)).
    ByteAligned,
}

impl Alignment {
    fn to_rexi(self) -> rexi_core::Alignment {
        match self {
            Alignment::BitPacked => rexi_core::Alignment::BitPacked,
            Alignment::ByteAligned => rexi_core::Alignment::ByteAligned,
        }
    }
}

/// The options every SenML EXI stream uses: strict, schemaId "a"
/// ([RFC 8428 section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8),
/// [EXI 5.4](https://www.w3.org/TR/exi/#options)). Everything else keeps its EXI
/// default.
fn exi_options(alignment: Alignment) -> rexi_core::ExiOptions {
    rexi_core::ExiOptions::strict()
        .with_alignment(alignment.to_rexi())
        .with_schema_id(rsenml_core::EXI_SCHEMA_ID)
}

/// rexi's errors in SenML terms. `bit` is the bit position where the error was
/// found; the error reports the byte that holds it.
fn exi_error(e: rexi_core::ExiError, bit: u64) -> rsenml_core::Error {
    use rsenml_core::{Error, Format};
    match e {
        rexi_core::ExiError::Truncated => Error::Incomplete {
            format: Format::Exi,
        },
        rexi_core::ExiError::Io(rexi_core::IoErrorKind::BufferFull) => Error::BufferFull,
        // rexi checks the header's schemaId against the tables' id ("a").
        rexi_core::ExiError::SchemaIdMismatch { stream, .. } => Error::SchemaId {
            found: match *stream {
                rexi_core::SchemaId::Id(id) => Some(id),
                _ => None,
            },
        },
        e @ (rexi_core::ExiError::Header(_) | rexi_core::ExiError::Grammar { .. }) => {
            Error::Syntax {
                format: Format::Exi,
                offset: (bit / 8) as usize,
                reason: exi_reason(e).into(),
            }
        }
        e => Error::Exi(exi_reason(e)),
    }
}

/// rexi's error as text, built without `core::fmt` (formatting it would pull
/// `core::fmt` into every build). Each message is up to four fixed pieces joined.
fn exi_reason(e: rexi_core::ExiError) -> alloc::string::String {
    use rexi_core::ExiError as E;
    let parts: [&str; 4] = match &e {
        E::Io(_) => ["I/O error", "", "", ""],
        E::Truncated => ["stream ended prematurely", "", "", ""],
        E::Header(m) => ["invalid EXI header: ", m, "", ""],
        E::Unsupported(_) => ["unsupported EXI feature", "", "", ""],
        E::Grammar { expected, got } => ["grammar mismatch: expected ", expected, ", got ", got],
        E::Value { datatype, reason } => ["value not representable as ", datatype, ": ", reason],
        E::StringTable(m) => ["string table error: ", m, "", ""],
        E::Schema(m) => ["schema error: ", m, "", ""],
        E::Internal(m) => ["internal error: ", m, "", ""],
        E::LimitExceeded { .. } => ["decoder limit exceeded", "", "", ""],
        // An error kind added to rexi later.
        _ => ["EXI error", "", "", ""],
    };
    parts.concat()
}
