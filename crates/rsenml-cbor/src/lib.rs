//! The CBOR format of SenML
//! ([RFC 8428 section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)): media
//! types `application/senml+cbor` and, for streams, `application/sensml+cbor`
//! ([section 12.3](https://www.rfc-editor.org/rfc/rfc8428#section-12.3)). CBOR
//! itself is defined in [RFC 8949](https://www.rfc-editor.org/rfc/rfc8949).
//!
//! ```
//! use rsenml_core::{Pack, Record};
//! let pack: Pack = [
//!     Record::new().with_name("urn:dev:ow:10e2073a01080063").with_unit("Cel").with_value(23.1),
//! ].into_iter().collect();
//! let bytes = rsenml_cbor::encode(&pack)?;
//! assert_eq!(rsenml_cbor::decode(&bytes)?, pack);
//! # Ok::<(), rsenml_core::Error>(())
//! ```
//!
//! Decoded text and byte strings borrow from the input. `no_std`; needs `alloc`.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

mod read;
mod write;

pub use read::{RecordReader, StreamDecoder, decode, decode_sensml, decode_unchecked};
pub use write::{
    StreamEncoder, encode, encode_into, encode_record, encode_sensml, encode_unchecked,
};
