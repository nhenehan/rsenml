//! The JSON format of SenML
//! ([RFC 8428 section 5](https://www.rfc-editor.org/rfc/rfc8428#section-5)): media
//! types `application/senml+json` and, for streams, `application/sensml+json`
//! ([section 12.3](https://www.rfc-editor.org/rfc/rfc8428#section-12.3)).
//!
//! ```
//! let pack = rsenml_json::decode(
//!     br#"[{"bn":"urn:dev:ow:10e2073a01080063:","n":"voltage","u":"V","v":120.1},
//!          {"n":"current","u":"A","v":1.2}]"#,
//! )?;
//! assert_eq!(pack.records[1].name.as_deref(), Some("current"));
//! assert_eq!(
//!     rsenml_json::encode(&pack)?,
//!     br#"[{"bn":"urn:dev:ow:10e2073a01080063:","n":"voltage","u":"V","v":120.1},{"n":"current","u":"A","v":1.2}]"#
//! );
//! # Ok::<(), rsenml_core::Error>(())
//! ```
//!
//! Reading checks the JSON grammar (with nanojson), label types, repeated labels,
//! at most one value per record, and labels ending in `_`. [`decode`] then
//! validates the pack
//! ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
//! Unknown labels with string, number or boolean values are kept as
//! [`rsenml_core::Extension`]s. Other unknown labels are skipped.
//!
//! `no_std`; needs `alloc`.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

mod read;
mod write;

pub use read::{RecordReader, StreamDecoder, decode, decode_unchecked};
pub use write::{StreamEncoder, encode, encode_into, encode_record, encode_unchecked};
