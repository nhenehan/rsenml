//! Rules of
//! [RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4) that
//! cover the whole pack, which no single field can check alone.
//!
//! How rsenml reads the spec where it leaves room (also listed in `doc/design.md`):
//! - A record with only base fields sets values for the records after it. It needs
//!   no value or name (see the first record of the example in
//!   [section 5.1.7](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.7)). Every
//!   other record needs a value or a sum
//!   ([section 4.2](https://www.rfc-editor.org/rfc/rfc8428#section-4.2)).
//! - Version: each record's version is the last `bver` at or before it (10 when
//!   there is none). All records must agree
//!   ([section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)), so a
//!   `bver` that differs from the version already in force is an error. Versions
//!   above 10 must not be used. Versions below 10 are accepted (the RFC's own
//!   example uses 5).
//! - Name rules apply to measurement records only: base name + name must not be
//!   empty and must follow
//!   [section 4.5.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1).

use alloc::string::ToString;

use crate::error::{Error, Result};
use crate::label::{Label, is_must_understand};
use crate::record::{Pack, Record, Value};
use crate::units;

/// The SenML version this crate implements, which is also the default when there
/// is no `bver`
/// ([section 4.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.1)).
pub const SUPPORTED_VERSION: u64 = 10;

/// Optional checks beyond what RFC 8428 requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ValidationOptions {
    /// Reject units (`u`, `bu`) that are not in the SenML Units registry (Table 6,
    /// [section 12.1](https://www.rfc-editor.org/rfc/rfc8428#section-12.1)).
    /// RFC 8428 does not require registered units, so this is off by default.
    pub registered_units: bool,
}

impl ValidationOptions {
    /// The RFC 8428 rules only (the default).
    pub const fn new() -> Self {
        Self {
            registered_units: false,
        }
    }

    /// Also require registered units ([`ValidationOptions::registered_units`]).
    pub const fn with_registered_units(mut self, on: bool) -> Self {
        self.registered_units = on;
        self
    }
}

/// Checks the records of a pack one at a time, so a SenSML stream can be checked
/// as its records arrive. Call [`finish`](Self::finish) at the end of the pack.
#[derive(Debug, Clone)]
pub struct Validator {
    options: ValidationOptions,
    index: usize,
    version: Option<u64>,
    /// What the name rule needs to know about the base name in force. It is worked
    /// out once, when a record sets the base name (not again for each record), so
    /// the text itself is not kept.
    base: Option<BaseName>,
}

/// The base name in force, reduced to what
/// [section 4.5.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1) needs.
#[derive(Debug, Clone, Copy)]
struct BaseName {
    empty: bool,
    /// Its first character may start a name, and every character may appear in one.
    valid: bool,
}

impl BaseName {
    fn of(bn: &str) -> Self {
        let b = bn.as_bytes();
        Self {
            empty: b.is_empty(),
            valid: b.first().is_some_and(|&c| NAME_START[usize::from(c)])
                && b.iter().all(|&c| NAME_CHAR[usize::from(c)]),
        }
    }
}

/// Characters allowed in a name
/// ([section 4.5.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1)):
/// `A-Za-z0-9:./_-`. A 256-entry lookup table, one entry per byte value; every
/// non-ASCII byte is `false`.
const NAME_CHAR: [bool; 256] = {
    let mut t = [false; 256];
    let mut c = 0;
    while c < 256 {
        let b = c as u8;
        t[c] = b.is_ascii_alphanumeric() || matches!(b, b'-' | b':' | b'.' | b'/' | b'_');
        c += 1;
    }
    t
};

/// Characters a name may start with: `A-Za-z0-9`.
const NAME_START: [bool; 256] = {
    let mut t = [false; 256];
    let mut c = 0;
    while c < 256 {
        t[c] = (c as u8).is_ascii_alphanumeric();
        c += 1;
    }
    t
};

impl Validator {
    /// A validator for one pack or stream.
    pub fn new(options: ValidationOptions) -> Self {
        Self {
            options,
            index: 0,
            version: None,
            base: None,
        }
    }

    /// Checks the next record of the pack.
    pub fn check(&mut self, record: &Record<'_>) -> Result<()> {
        let index = self.index;
        self.index += 1;
        self.check_fields(record, index)?;
        self.check_version(record, index)?;
        if let Some(bn) = &record.base_name {
            self.base = Some(BaseName::of(bn));
        }
        if record.is_base_only() {
            return Ok(());
        }
        if record.value.is_none() && record.sum.is_none() {
            return Err(Error::MissingValue { record: index });
        }
        let n = match (&record.name, self.base) {
            (None, None) => return Err(Error::MissingName { record: index }),
            (name, _) => name.as_deref().unwrap_or("").as_bytes(),
        };
        let valid = match self.base {
            // The name is bn + n. A non-empty bn holds the first character, and it was
            // checked when it was set. So n must only hold name characters.
            Some(bn) if !bn.empty => bn.valid && n.iter().all(|&c| NAME_CHAR[usize::from(c)]),
            // No base name, or an empty one: n alone must follow the whole rule.
            _ => {
                n.first().is_some_and(|&c| NAME_START[usize::from(c)])
                    && n.iter().all(|&c| NAME_CHAR[usize::from(c)])
            }
        };
        if !valid {
            return Err(Error::InvalidName { record: index });
        }
        if let Some(unit) = &record.unit
            && self.options.registered_units
            && units::lookup(unit).is_none()
        {
            return Err(Error::UnregisteredUnit { record: index });
        }
        Ok(())
    }

    /// Ends the pack. It must have held at least one record (the CDDL is
    /// `[1* record]`,
    /// [section 11](https://www.rfc-editor.org/rfc/rfc8428#section-11)).
    pub fn finish(&self) -> Result<()> {
        if self.index == 0 {
            Err(Error::EmptyPack)
        } else {
            Ok(())
        }
    }

    fn check_fields(&self, record: &Record<'_>, index: usize) -> Result<()> {
        // All numbers must be finite. Run a cheap test first, and look up the label
        // only when it fails.
        let fixed = [
            record.base_time,
            record.base_value,
            record.base_sum,
            record.value.as_ref().and_then(Value::as_number),
            record.sum,
            record.time,
            record.update_time,
        ];
        if (fixed.iter().flatten().any(|v| !v.is_finite())
            || record
                .extensions
                .iter()
                .any(|e| e.value.as_number().is_some_and(|v| !v.is_finite())))
            && let Some((label, _)) = record.numbers().find(|(_, v)| !v.is_finite())
        {
            return Err(Error::NonFinite {
                record: index,
                label: label.to_string(),
            });
        }
        for (i, ext) in record.extensions.iter().enumerate() {
            if is_must_understand(&ext.label) {
                return Err(Error::MustUnderstand {
                    record: index,
                    label: ext.label.to_string(),
                });
            }
            if Label::from_text(&ext.label).is_some() {
                return Err(Error::InvalidLabel {
                    record: index,
                    label: ext.label.to_string(),
                });
            }
            if record.extensions[..i].iter().any(|e| e.label == ext.label) {
                return Err(Error::DuplicateLabel {
                    record: index,
                    label: ext.label.to_string(),
                });
            }
        }
        if let Some(bu) = &record.base_unit
            && self.options.registered_units
            && units::lookup(bu).is_none()
        {
            return Err(Error::UnregisteredUnit { record: index });
        }
        Ok(())
    }

    fn check_version(&mut self, record: &Record<'_>, index: usize) -> Result<()> {
        let Some(bver) = record.base_version else {
            return Ok(());
        };
        if bver == 0 {
            return Err(Error::InvalidVersion { record: index });
        }
        if bver > SUPPORTED_VERSION {
            return Err(Error::UnsupportedVersion {
                record: index,
                version: bver,
            });
        }
        let in_force = match self.version {
            Some(v) => v,
            // Records before the first bver had the default version (10).
            None if index > 0 => SUPPORTED_VERSION,
            None => bver,
        };
        if in_force != bver {
            return Err(Error::InconsistentVersion { record: index });
        }
        self.version = Some(bver);
        Ok(())
    }
}

/// [Section 4.5.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1): the joined
/// name `bn + n` must match `[A-Za-z0-9][A-Za-z0-9:./_-]*`.
pub fn valid_name(base_name: &str, name: &str) -> bool {
    let mut chars = base_name.bytes().chain(name.bytes());
    let Some(first) = chars.next() else {
        return false;
    };
    NAME_START[usize::from(first)] && chars.all(|c| NAME_CHAR[usize::from(c)])
}

impl Pack<'_> {
    /// Checks the rules of
    /// [RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4) (see
    /// [`Validator`]).
    pub fn validate(&self) -> Result<()> {
        self.validate_with(&ValidationOptions::default())
    }

    /// [`Pack::validate`] with optional checks ([`ValidationOptions`]).
    pub fn validate_with(&self, options: &ValidationOptions) -> Result<()> {
        let mut v = Validator::new(*options);
        for record in self {
            v.check(record)?;
        }
        v.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Value;
    use alloc::vec;

    fn pack(records: alloc::vec::Vec<Record<'static>>) -> Pack<'static> {
        Pack::from(records)
    }

    #[test]
    fn base_only_record_is_context() {
        // RFC 8428 section 5.1.7.
        let p = pack(vec![
            Record::new().with_base_name("urn:dev:ow:10e2073a01080063:"),
            Record::new()
                .with_name("temp")
                .with_unit("Cel")
                .with_value(23.1),
        ]);
        assert_eq!(p.validate(), Ok(()));
    }

    #[test]
    fn value_and_name_rules() {
        assert_eq!(
            pack(vec![Record::new().with_name("a").with_time(1.0)]).validate(),
            Err(Error::MissingValue { record: 0 })
        );
        assert_eq!(
            pack(vec![Record::new().with_name("a").with_sum(1.0)]).validate(),
            Ok(())
        );
        assert_eq!(
            pack(vec![Record::new().with_value(1.0)]).validate(),
            Err(Error::MissingName { record: 0 })
        );
        assert_eq!(
            pack(vec![Record::new().with_name("-x").with_value(1.0)]).validate(),
            Err(Error::InvalidName { record: 0 })
        );
        assert_eq!(
            pack(vec![Record::new().with_name("a b").with_value(1.0)]).validate(),
            Err(Error::InvalidName { record: 0 })
        );
        assert_eq!(
            pack(vec![Record::new().with_base_name("").with_value(1.0)]).validate(),
            Err(Error::InvalidName { record: 0 })
        );
        assert_eq!(pack(vec![]).validate(), Err(Error::EmptyPack));
    }

    #[test]
    fn versions() {
        let r = |v: Option<u64>| {
            let mut r = Record::new().with_name("a").with_value(1.0);
            r.base_version = v;
            r
        };
        assert_eq!(pack(vec![r(Some(5)), r(None)]).validate(), Ok(()));
        assert_eq!(pack(vec![r(Some(5)), r(Some(5))]).validate(), Ok(()));
        assert_eq!(
            pack(vec![r(Some(5)), r(Some(10))]).validate(),
            Err(Error::InconsistentVersion { record: 1 })
        );
        assert_eq!(
            pack(vec![r(None), r(Some(5))]).validate(),
            Err(Error::InconsistentVersion { record: 1 })
        );
        assert_eq!(pack(vec![r(None), r(Some(10))]).validate(), Ok(()));
        assert_eq!(
            pack(vec![r(Some(11))]).validate(),
            Err(Error::UnsupportedVersion {
                record: 0,
                version: 11
            })
        );
        assert_eq!(
            pack(vec![r(Some(0))]).validate(),
            Err(Error::InvalidVersion { record: 0 })
        );
    }

    #[test]
    fn extensions_and_units() {
        let p = pack(vec![
            Record::new()
                .with_name("a")
                .with_value(1.0)
                .with_extension("crit_", Value::Bool(true)),
        ]);
        assert!(matches!(p.validate(), Err(Error::MustUnderstand { .. })));
        let p = pack(vec![
            Record::new()
                .with_name("a")
                .with_unit("furlong")
                .with_value(1.0),
        ]);
        assert_eq!(p.validate(), Ok(()));
        let strict = ValidationOptions {
            registered_units: true,
        };
        assert_eq!(
            p.validate_with(&strict),
            Err(Error::UnregisteredUnit { record: 0 })
        );
        let p = pack(vec![
            Record::new()
                .with_name("a")
                .with_unit("Cel")
                .with_value(1.0),
        ]);
        assert_eq!(p.validate_with(&strict), Ok(()));
    }

    #[test]
    fn name_rule_matches_the_concatenation_check() {
        // Base names are set, changed and emptied along the pack. Names have valid and
        // invalid characters in every position. The validator's answer must match
        // valid_name(bn + n), the rule as the spec writes it.
        let alphabet = ["", "a", "Z9", "-x", ":", "dev/", "a b", "_", "é", "0.1"];
        let mut state = 0x1234_5678u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..5000 {
            let mut bn: Option<&str> = None;
            for _ in 0..4 {
                let r = next();
                let mut record = Record::new().with_value(1.0);
                if r % 3 == 0 {
                    let b = alphabet[(r >> 8) as usize % alphabet.len()];
                    record.base_name = Some(b.into());
                    bn = Some(b);
                }
                let name = if (r >> 16) % 4 == 0 {
                    None
                } else {
                    Some(alphabet[(r >> 24) as usize % alphabet.len()])
                };
                record.name = name.map(Into::into);
                let mut v = Validator::new(ValidationOptions::default());
                // Replay the base name in force so far, then this record.
                if let Some(b) = bn {
                    let _ = v.check(&Record::new().with_base_name(b));
                }
                let got = v.check(&record);
                let expected = match (name, bn) {
                    (None, None) => Err(Error::MissingName { record: 1 }),
                    (n, b) if valid_name(b.unwrap_or(""), n.unwrap_or("")) => Ok(()),
                    _ => Err(Error::InvalidName { record: 1 }),
                };
                let got = got.map_err(|e| match e {
                    Error::MissingName { .. } => Error::MissingName { record: 1 },
                    Error::InvalidName { .. } => Error::InvalidName { record: 1 },
                    other => other,
                });
                assert_eq!(got, expected, "bn {bn:?} n {name:?}");
            }
        }
    }
}
