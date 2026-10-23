//! The XML format of SenML
//! ([RFC 8428 section 7](https://www.rfc-editor.org/rfc/rfc8428#section-7)): media
//! types `application/senml+xml` and, for streams, `application/sensml+xml`
//! ([section 12.3](https://www.rfc-editor.org/rfc/rfc8428#section-12.3)).
//!
//! ```
//! let pack = rsenml_xml::decode(
//!     br#"<sensml xmlns="urn:ietf:params:xml:ns:senml">
//!           <senml bn="urn:dev:ow:10e2073a01080063:" n="voltage" u="V" v="120.1"></senml>
//!           <senml n="current" u="A" v="1.2"></senml>
//!         </sensml>"#,
//! )?;
//! assert_eq!(pack.records[0].value, Some(rsenml_core::Value::Number(120.1)));
//! assert_eq!(
//!     rsenml_xml::encode(&pack)?,
//!     br#"<sensml xmlns="urn:ietf:params:xml:ns:senml"><senml bn="urn:dev:ow:10e2073a01080063:" n="voltage" u="V" v="120.1"/><senml n="current" u="A" v="1.2"/></sensml>"#
//! );
//! # Ok::<(), rsenml_core::Error>(())
//! ```
//!
//! `no_std`; needs `alloc`.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

mod read;
mod write;

pub use read::{RecordReader, decode, decode_unchecked};
pub use write::{StreamEncoder, encode, encode_into, encode_record, encode_unchecked};
