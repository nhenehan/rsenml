//! The data model of RFC 8428, Sensor Measurement Lists (SenML). The rsenml codecs
//! (JSON, CBOR, XML, EXI) all share it.
//!
//! - [`Record`] and [`Pack`]: records as written, base fields included. A *pack* is
//!   the whole list of records. *Base fields* (`bn`, `bt`, ...) set values for the
//!   records that follow
//!   ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4));
//! - [`Pack::validate`] / [`Validator`]: the rules of
//!   [section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4);
//! - [`Pack::resolve`] / [`Resolver`]: resolved records, that is, records with the
//!   base fields already applied
//!   ([section 4.6](https://www.rfc-editor.org/rfc/rfc8428#section-4.6));
//! - [`units`]: the SenML Units registry
//!   ([section 12.1](https://www.rfc-editor.org/rfc/rfc8428#section-12.1));
//! - [`fragment`]: `rec=` fragment identifiers
//!   ([section 9](https://www.rfc-editor.org/rfc/rfc8428#section-9));
//! - [`MediaType`]: media types and CoAP Content-Formats (sections
//!   [12.3](https://www.rfc-editor.org/rfc/rfc8428#section-12.3) and
//!   [12.5](https://www.rfc-editor.org/rfc/rfc8428#section-12.5));
//! - [`base64url`]: the text form of the Data Value (`vd`).
//!
//! The `codec` module holds the parts that the rsenml codec crates share (the record
//! builder and the number conversions). It is not part of the stable API.
//!
//! `no_std`; needs `alloc`. No dependencies.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

pub mod base64url;
mod builder;
#[doc(hidden)]
pub mod codec;
mod error;
pub mod fragment;
mod label;
mod media;
mod number;
mod record;
mod resolve;
pub mod units;
mod validate;

pub use error::{Error, Format, Result};
pub use label::{Label, LabelType, is_base_label_text, is_must_understand};
pub use media::MediaType;
pub use record::{Extension, Pack, Record, Value};
pub use resolve::{ABSOLUTE_TIME_THRESHOLD, ResolvedRecord, Resolver, is_relative_time};
pub use validate::{SUPPORTED_VERSION, ValidationOptions, Validator, valid_name};

/// The XML namespace of SenML
/// ([RFC 8428 section 12.4](https://www.rfc-editor.org/rfc/rfc8428#section-12.4)).
pub const XML_NAMESPACE: &str = "urn:ietf:params:xml:ns:senml";

/// The EXI schemaId of the RFC 8428 schema. The EXI header options must carry it
/// ([RFC 8428 section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8)).
pub const EXI_SCHEMA_ID: &str = "a";
