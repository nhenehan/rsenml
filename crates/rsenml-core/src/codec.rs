//! Parts shared by the rsenml codec crates: the record builder that every decoder
//! uses, and the number conversions. This is public only because the codecs are
//! separate crates. It is **not covered by semver** and may change in any release.

pub use crate::builder::{FieldValue, RecordBuilder};
/// Number conversions (shortest text, EXI decimal parts, CBOR float widths).
pub mod number {
    pub use crate::number::*;
}
