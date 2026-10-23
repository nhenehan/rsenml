//! The error type every rsenml codec reports.

use alloc::borrow::Cow;
use alloc::string::String;
use core::fmt;

/// Which representation an error was found in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// [RFC 8428 section 5](https://www.rfc-editor.org/rfc/rfc8428#section-5).
    Json,
    /// [RFC 8428 section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6).
    Cbor,
    /// [RFC 8428 section 7](https://www.rfc-editor.org/rfc/rfc8428#section-7).
    Xml,
    /// [RFC 8428 section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8).
    Exi,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Format::Json => "JSON",
            Format::Cbor => "CBOR",
            Format::Xml => "XML",
            Format::Exi => "EXI",
        })
    }
}

/// Everything that can go wrong reading, checking, resolving or writing SenML.
///
/// `record` fields are 0-based positions in the pack. `offset` fields are byte
/// offsets into the input. EXI works in bits, so it reports the byte that holds the
/// bit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The input is not well-formed in its representation (JSON, CBOR, XML or EXI).
    Syntax {
        /// The representation being read or written.
        format: Format,
        /// Byte offset into the input where the problem was found.
        offset: usize,
        /// What is wrong, in words.
        reason: Cow<'static, str>,
    },
    /// The input ended before the pack did. For a SenSML stream, this means: wait for
    /// more bytes.
    Incomplete {
        /// The representation being read or written.
        format: Format,
    },
    /// A registered label carries a value of the wrong type.
    WrongType {
        /// 0-based position of the record in the pack.
        record: usize,
        /// The label concerned.
        label: &'static str,
    },
    /// A label occurs twice in one record.
    DuplicateLabel {
        /// 0-based position of the record in the pack.
        record: usize,
        /// The label concerned.
        label: String,
    },
    /// More than one of `v`, `vs`, `vb`, `vd` in one record
    /// ([section 4.2](https://www.rfc-editor.org/rfc/rfc8428#section-4.2)).
    MultipleValues {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// A label ending in `_` that this implementation does not understand
    /// ([section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)).
    MustUnderstand {
        /// 0-based position of the record in the pack.
        record: usize,
        /// The label concerned.
        label: String,
    },
    /// A label that the format keeps for a registered label is used in another way
    /// (for example, an extension named `"v"` in a record built by hand).
    InvalidLabel {
        /// 0-based position of the record in the pack.
        record: usize,
        /// The label concerned.
        label: String,
    },
    /// A Data Value that is not base64url without padding (sections
    /// [5](https://www.rfc-editor.org/rfc/rfc8428#section-5) and
    /// [7](https://www.rfc-editor.org/rfc/rfc8428#section-7)).
    InvalidBase64 {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// A number that is infinite or NaN. JSON cannot carry these, so SenML does not.
    NonFinite {
        /// 0-based position of the record in the pack.
        record: usize,
        /// The label concerned.
        label: String,
    },
    /// A pack must hold at least one record.
    EmptyPack,
    /// A record with regular fields but neither a value nor a sum
    /// ([section 4.2](https://www.rfc-editor.org/rfc/rfc8428#section-4.2)).
    MissingValue {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// A measurement record with no name and no base name
    /// ([section 4.2](https://www.rfc-editor.org/rfc/rfc8428#section-4.2)).
    MissingName {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// The joined name (base name + name) breaks the rules of
    /// [section 4.5.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1).
    InvalidName {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// A Base Version that is not a positive integer.
    InvalidVersion {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// A Base Version newer than this implementation (10). The pack must not be used
    /// ([section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)).
    UnsupportedVersion {
        /// 0-based position of the record in the pack.
        record: usize,
        /// The version found in `bver`.
        version: u64,
    },
    /// Records of one pack with different versions
    /// ([section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)).
    InconsistentVersion {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// A unit not in the SenML Units registry (only with `ValidationOptions::registered_units`).
    UnregisteredUnit {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// Resolution found a relative time
    /// ([section 4.5.3](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.3)), and
    /// no reference time for "now" was given.
    RelativeTime {
        /// 0-based position of the record in the pack.
        record: usize,
    },
    /// The SenML XML/EXI root or record element is missing or wrong.
    Structure {
        /// The representation being read or written.
        format: Format,
        /// What is wrong, in words.
        reason: &'static str,
    },
    /// The EXI stream does not name the RFC 8428 schema (schemaId "a",
    /// [section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8)).
    SchemaId {
        /// The schemaId the header names, if it names one.
        found: Option<String>,
    },
    /// The value or feature cannot be written in this representation.
    Unrepresentable {
        /// The representation being read or written.
        format: Format,
        /// What is wrong, in words.
        reason: Cow<'static, str>,
    },
    /// A fixed output buffer is full.
    BufferFull,
    /// An error from the EXI codec that has no SenML meaning.
    Exi(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Syntax {
                format,
                offset,
                reason,
            } => write!(f, "invalid {format} at byte {offset}: {reason}"),
            Error::Incomplete { format } => write!(f, "{format} input ends inside the pack"),
            Error::WrongType { record, label } => {
                write!(f, "record {record}: wrong value type for label {label}")
            }
            Error::DuplicateLabel { record, label } => {
                write!(f, "record {record}: label {label} occurs twice")
            }
            Error::MultipleValues { record } => {
                write!(f, "record {record}: more than one of v, vs, vb, vd")
            }
            Error::MustUnderstand { record, label } => write!(
                f,
                "record {record}: label {label} must be understood and is not supported"
            ),
            Error::InvalidLabel { record, label } => {
                write!(f, "record {record}: label {label} is not valid here")
            }
            Error::InvalidBase64 { record } => {
                write!(f, "record {record}: vd is not unpadded base64url")
            }
            Error::NonFinite { record, label } => {
                write!(f, "record {record}: {label} is not a finite number")
            }
            Error::EmptyPack => f.write_str("a SenML pack needs at least one record"),
            Error::MissingValue { record } => {
                write!(f, "record {record}: neither a value nor a sum")
            }
            Error::MissingName { record } => {
                write!(f, "record {record}: neither a name nor a base name")
            }
            Error::InvalidName { record } => write!(
                f,
                "record {record}: name must be [A-Za-z0-9][A-Za-z0-9:./_-]* (RFC 8428 4.5.1)"
            ),
            Error::InvalidVersion { record } => {
                write!(f, "record {record}: bver must be a positive integer")
            }
            Error::UnsupportedVersion { record, version } => write!(
                f,
                "record {record}: SenML version {version} is newer than 10 and must not be used"
            ),
            Error::InconsistentVersion { record } => {
                write!(
                    f,
                    "record {record}: version differs from the rest of the pack"
                )
            }
            Error::UnregisteredUnit { record } => {
                write!(
                    f,
                    "record {record}: unit is not in the SenML Units registry"
                )
            }
            Error::RelativeTime { record } => write!(
                f,
                "record {record}: relative time and no reference time to resolve it"
            ),
            Error::Structure { format, reason } => write!(f, "invalid SenML {format}: {reason}"),
            Error::SchemaId { found: Some(id) } => {
                write!(f, "EXI schemaId {id:?} is not the RFC 8428 schema \"a\"")
            }
            Error::SchemaId { found: None } => {
                f.write_str("EXI header carries no options with schemaId \"a\"")
            }
            Error::Unrepresentable { format, reason } => {
                write!(f, "cannot write as {format}: {reason}")
            }
            Error::BufferFull => f.write_str("output buffer full"),
            Error::Exi(msg) => write!(f, "EXI: {msg}"),
        }
    }
}

impl core::error::Error for Error {}

/// `Result` with this crate's [`Error`].
pub type Result<T> = core::result::Result<T, Error>;
