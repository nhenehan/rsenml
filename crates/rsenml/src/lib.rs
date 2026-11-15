//! Read and write [SenML](https://www.rfc-editor.org/rfc/rfc8428) (Sensor
//! Measurement Lists, RFC 8428) and its streaming form SenSML
//! ([section 4.8](https://www.rfc-editor.org/rfc/rfc8428#section-4.8)) in all
//! four formats: JSON, CBOR, XML and EXI.
//!
//! A SenML message is a [`Pack`] of [`Record`]s. Each record has a name, a value,
//! and maybe a unit and a time. Base fields (`bn`, `bt`, `bu`, ...) set values for
//! the records after them, so a long device name is written only once.
//!
//! ```
//! use rsenml::{MediaType, Pack, Record};
//!
//! let pack: Pack = [
//!     Record::new().with_base_name("urn:dev:ow:10e2073a01080063:").with_name("temp")
//!         .with_unit("Cel").with_value(23.1),
//!     Record::new().with_name("open").with_bool_value(false),
//! ].into_iter().collect();
//!
//! for media in [MediaType::SenmlJson, MediaType::SenmlCbor, MediaType::SenmlXml, MediaType::SenmlExi] {
//!     let bytes = rsenml::encode(media, &pack)?;
//!     assert_eq!(rsenml::decode(media, &bytes)?, pack);
//! }
//! let resolved = pack.resolve(Some(1_700_000_000.0))?;
//! assert_eq!(resolved[0].name, "urn:dev:ow:10e2073a01080063:temp");
//! # Ok::<(), rsenml::Error>(())
//! ```
//!
//! The [user guide](https://nhenehan.github.io/rsenml/) walks through one small
//! program per idea.
//!
//! # Formats
//!
//! Each format is a module here and a feature of this crate (all on by default):
//!
//! | Module | Feature | Media types | RFC 8428 |
//! |---|---|---|---|
//! | [`json`] | `json` | `application/senml+json`, `sensml+json` | [section 5](https://www.rfc-editor.org/rfc/rfc8428#section-5) |
//! | [`cbor`] | `cbor` | `application/senml+cbor`, `sensml+cbor` | [section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6) |
//! | [`xml`] | `xml` | `application/senml+xml`, `sensml+xml` | [section 7](https://www.rfc-editor.org/rfc/rfc8428#section-7) |
//! | [`exi`] | `exi` | `application/senml-exi`, `sensml-exi` | [section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8) |
//!
//! [`decode`] and [`encode`] pick the format from a [`MediaType`], which also
//! converts to and from content-type text and CoAP Content-Format numbers.
//!
//! # Reading and writing
//!
//! Every format module has the same shape:
//!
//! - `decode` and `encode` for whole packs. Both validate the pack; the
//!   `_unchecked` versions don't, for packs already known to be valid.
//! - `encode_into` writes into a fixed buffer and returns the length, or
//!   [`Error::BufferFull`].
//! - `RecordReader` reads one record at a time from a whole message.
//! - `StreamEncoder` writes one record at a time, for SenSML.
//! - JSON and CBOR also have `StreamDecoder`, which takes bytes as they arrive and
//!   gives each record once it is complete.
//!
//! # Checking and resolving
//!
//! [`Pack::validate`] checks the rules of
//! [section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4);
//! [`ValidationOptions`] can also require registered units ([`units`]).
//! [`Pack::resolve`] turns records into [`ResolvedRecord`]s with full names,
//! units and absolute times
//! ([section 4.6](https://www.rfc-editor.org/rfc/rfc8428#section-4.6)).
//! [`Validator`] and [`Resolver`] do the same one record at a time.
//! [`fragment`] reads `rec=` fragment identifiers.
//!
//! # Errors
//!
//! Every call returns [`Result`], with one [`Error`] type for all formats. Errors
//! about a record say which one, counted from 0.
//!
//! # `no_std`
//!
//! This crate and the ones under it are `#![no_std]` and need only `alloc`. They
//! build for Cortex-M (`thumbv7em-none-eabihf`) and forbid `unsafe` code. Turn off
//! the formats you don't use to keep the build small:
//!
//! ```toml
//! rsenml = { version = "0.9", default-features = false, features = ["cbor"] }
//! ```

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

use alloc::vec::Vec;

pub use rsenml_core::*;

#[cfg(feature = "cbor")]
pub use rsenml_cbor as cbor;
#[cfg(feature = "exi")]
pub use rsenml_exi as exi;
#[cfg(feature = "json")]
pub use rsenml_json as json;
#[cfg(feature = "xml")]
pub use rsenml_xml as xml;

/// Reads and validates a pack of the given media type. The SenSML types accept
/// streams (for CBOR, an indefinite-length array).
pub fn decode(media: MediaType, input: &[u8]) -> Result<Pack<'_>> {
    let _ = input; // unused when no format is enabled
    match media.format() {
        #[cfg(feature = "json")]
        Format::Json => json::decode(input),
        #[cfg(feature = "cbor")]
        Format::Cbor if media.is_streaming() => cbor::decode_sensml(input),
        #[cfg(feature = "cbor")]
        Format::Cbor => cbor::decode(input),
        #[cfg(feature = "xml")]
        Format::Xml => xml::decode(input),
        #[cfg(feature = "exi")]
        Format::Exi => exi::decode(input),
        #[allow(unreachable_patterns)]
        format => Err(not_enabled(format)),
    }
}

/// Validates and writes a pack in the given media type. SenSML CBOR uses an
/// indefinite-length array. The other SenSML types are written the same as SenML.
pub fn encode(media: MediaType, pack: &Pack<'_>) -> Result<Vec<u8>> {
    let _ = pack; // unused when no format is enabled
    match media.format() {
        #[cfg(feature = "json")]
        Format::Json => json::encode(pack),
        #[cfg(feature = "cbor")]
        Format::Cbor if media.is_streaming() => cbor::encode_sensml(pack),
        #[cfg(feature = "cbor")]
        Format::Cbor => cbor::encode(pack),
        #[cfg(feature = "xml")]
        Format::Xml => xml::encode(pack),
        #[cfg(feature = "exi")]
        Format::Exi => exi::encode(pack),
        #[allow(unreachable_patterns)]
        format => Err(not_enabled(format)),
    }
}

/// [`encode`] without validating the pack first, for packs that are already valid
/// (decoded, or built by trusted code).
pub fn encode_unchecked(media: MediaType, pack: &Pack<'_>) -> Result<Vec<u8>> {
    let _ = pack; // unused when no format is enabled
    match media.format() {
        #[cfg(feature = "json")]
        Format::Json => json::encode_unchecked(pack),
        #[cfg(feature = "cbor")]
        Format::Cbor if media.is_streaming() => {
            let mut out = Vec::new();
            let mut s = cbor::StreamEncoder::new();
            for r in pack {
                s.record(r, &mut out)?;
            }
            s.finish(&mut out);
            Ok(out)
        }
        #[cfg(feature = "cbor")]
        Format::Cbor => cbor::encode_unchecked(pack),
        #[cfg(feature = "xml")]
        Format::Xml => xml::encode_unchecked(pack),
        #[cfg(feature = "exi")]
        Format::Exi => exi::encode_unchecked(pack),
        #[allow(unreachable_patterns)]
        format => Err(not_enabled(format)),
    }
}

#[allow(dead_code)]
fn not_enabled(format: Format) -> Error {
    Error::Unrepresentable {
        format,
        reason: "this representation's feature is not enabled".into(),
    }
}
