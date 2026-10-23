//! Builds a record one field at a time. Every decoder shares it.
//!
//! The checks that belong to reading one record (rather than to validating a whole
//! pack) live here, so all formats apply them the same way: each label at most
//! once, at most one value label, numbers finite, and labels ending in `_` rejected
//! ([RFC 8428 section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)).

use alloc::borrow::Cow;
use alloc::string::ToString;

use crate::error::{Error, Result};
use crate::label::{Label, is_must_understand};
use crate::record::{Extension, Record, Value};

/// A decoded field value, before it is checked against its label's type.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue<'a> {
    /// Text.
    String(Cow<'a, str>),
    /// A number (any JSON number, any CBOR float or decimal fraction).
    Number(f64),
    /// A non-negative integer. Also accepted where a number is expected.
    Unsigned(u64),
    /// A boolean.
    Bool(bool),
    /// Octets (already decoded from base64url when the format uses text).
    Data(Cow<'a, [u8]>),
}

/// Builds one [`Record`] from decoded fields.
#[derive(Debug)]
pub struct RecordBuilder<'a> {
    record: Record<'a>,
    index: usize,
    seen: u16,
}

impl<'a> RecordBuilder<'a> {
    /// `index` is the record's position in the pack, for error reports.
    pub fn new(index: usize) -> Self {
        Self {
            record: Record::default(),
            index,
            seen: 0,
        }
    }

    /// The record's position in the pack.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Sets a registered label. The value must have the label's type. A
    /// [`FieldValue::Unsigned`] is accepted for numbers, and a [`FieldValue::Number`]
    /// holding a positive integer is accepted for `bver` (JSON has only one number
    /// type).
    pub fn set(&mut self, label: Label, value: FieldValue<'a>) -> Result<()> {
        // `seen` is a bit mask with one bit per registered label. The CBOR keys run
        // from -6 to 8, so key + 6 gives a bit number from 0 to 14.
        let bit = 1u16 << (label.cbor() + 6);
        if self.seen & bit != 0 {
            return Err(Error::DuplicateLabel {
                record: self.index,
                label: label.text().to_string(),
            });
        }
        self.seen |= bit;
        let index = self.index;
        let wrong = || Error::WrongType {
            record: index,
            label: label.text(),
        };
        let r = &mut self.record;
        match label {
            Label::BaseName => r.base_name = Some(string(value).ok_or_else(wrong)?),
            Label::BaseUnit => r.base_unit = Some(string(value).ok_or_else(wrong)?),
            Label::Name => r.name = Some(string(value).ok_or_else(wrong)?),
            Label::Unit => r.unit = Some(string(value).ok_or_else(wrong)?),
            Label::BaseTime => r.base_time = Some(number(index, label, value)?),
            Label::BaseValue => r.base_value = Some(number(index, label, value)?),
            Label::BaseSum => r.base_sum = Some(number(index, label, value)?),
            Label::Sum => r.sum = Some(number(index, label, value)?),
            Label::Time => r.time = Some(number(index, label, value)?),
            Label::UpdateTime => r.update_time = Some(number(index, label, value)?),
            Label::BaseVersion => r.base_version = Some(version(index, value)?),
            Label::Value => {
                let v = number(index, label, value)?;
                self.set_value(Value::Number(v))?;
            }
            Label::StringValue => {
                let s = string(value).ok_or_else(wrong)?;
                self.set_value(Value::String(s))?;
            }
            Label::BoolValue => match value {
                FieldValue::Bool(b) => self.set_value(Value::Bool(b))?,
                _ => return Err(wrong()),
            },
            Label::DataValue => match value {
                FieldValue::Data(d) => self.set_value(Value::Data(d))?,
                _ => return Err(wrong()),
            },
        }
        Ok(())
    }

    /// Adds a field whose label is not registered. Labels ending in `_` fail
    /// ([section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)). The
    /// value must be a scalar.
    pub fn extension(&mut self, label: Cow<'a, str>, value: FieldValue<'a>) -> Result<()> {
        self.check_extension_label(&label)?;
        let value = match value {
            FieldValue::String(s) => Value::String(s),
            FieldValue::Number(v) => {
                finite(self.index, &label, v)?;
                Value::Number(v)
            }
            FieldValue::Unsigned(u) => Value::Number(u as f64),
            FieldValue::Bool(b) => Value::Bool(b),
            FieldValue::Data(d) => Value::Data(d),
        };
        self.record.extensions.push(Extension { label, value });
        Ok(())
    }

    /// The checks of [`extension`](Self::extension) for a field whose value is
    /// skipped (an array or map, which the CDDL of
    /// [section 11](https://www.rfc-editor.org/rfc/rfc8428#section-11) does not
    /// allow). Must-understand labels (ending in `_`) still fail. Everything else is
    /// ignored.
    pub fn skipped_extension(&mut self, label: &str) -> Result<()> {
        if is_must_understand(label) {
            return Err(Error::MustUnderstand {
                record: self.index,
                label: label.to_string(),
            });
        }
        Ok(())
    }

    /// The record built.
    pub fn finish(self) -> Record<'a> {
        self.record
    }

    // An extension label must not end in `_`, must not be a registered label (that
    // is, the field was already given under its registered name), and must not
    // repeat an earlier extension.
    fn check_extension_label(&self, label: &str) -> Result<()> {
        if is_must_understand(label) {
            return Err(Error::MustUnderstand {
                record: self.index,
                label: label.to_string(),
            });
        }
        if Label::from_text(label).is_some()
            || self.record.extensions.iter().any(|e| e.label == label)
        {
            return Err(Error::DuplicateLabel {
                record: self.index,
                label: label.to_string(),
            });
        }
        Ok(())
    }

    fn set_value(&mut self, value: Value<'a>) -> Result<()> {
        if self.record.value.is_some() {
            return Err(Error::MultipleValues { record: self.index });
        }
        self.record.value = Some(value);
        Ok(())
    }
}

fn number(index: usize, label: Label, value: FieldValue<'_>) -> Result<f64> {
    let v = match value {
        FieldValue::Number(v) => v,
        FieldValue::Unsigned(u) => u as f64,
        _ => {
            return Err(Error::WrongType {
                record: index,
                label: label.text(),
            });
        }
    };
    finite(index, label.text(), v)?;
    Ok(v)
}

fn finite(index: usize, label: &str, v: f64) -> Result<()> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(Error::NonFinite {
            record: index,
            label: label.to_string(),
        })
    }
}

// `bver` from a number. A JSON number counts when it is a whole number from 0 to
// 2^53 (9,007,199,254,740,992), the range where every integer fits an IEEE 754
// double exactly. Zero passes here; the validator rejects it later.
fn version(index: usize, value: FieldValue<'_>) -> Result<u64> {
    match value {
        FieldValue::Unsigned(u) => Ok(u),
        FieldValue::Number(v)
            if (0.0..=9_007_199_254_740_992.0).contains(&v) && v == (v as u64) as f64 =>
        {
            Ok(v as u64)
        }
        FieldValue::Number(_) => Err(Error::InvalidVersion { record: index }),
        _ => Err(Error::WrongType {
            record: index,
            label: Label::BaseVersion.text(),
        }),
    }
}

fn string(value: FieldValue<'_>) -> Option<Cow<'_, str>> {
    match value {
        FieldValue::String(s) => Some(s),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicates_multiple_values_and_must_understand() {
        let mut b = RecordBuilder::new(3);
        assert_eq!(b.set(Label::Name, FieldValue::String("a".into())), Ok(()));
        assert_eq!(
            b.set(Label::Name, FieldValue::String("b".into())),
            Err(Error::DuplicateLabel {
                record: 3,
                label: "n".into()
            })
        );
        assert_eq!(b.set(Label::Value, FieldValue::Number(1.0)), Ok(()));
        assert_eq!(
            b.set(Label::BoolValue, FieldValue::Bool(true)),
            Err(Error::MultipleValues { record: 3 })
        );
        assert_eq!(
            b.extension("foo_".into(), FieldValue::Bool(true)),
            Err(Error::MustUnderstand {
                record: 3,
                label: "foo_".into()
            })
        );
        assert_eq!(b.extension("foo".into(), FieldValue::Bool(true)), Ok(()));
        assert!(b.extension("foo".into(), FieldValue::Bool(true)).is_err());
    }

    #[test]
    fn types_are_checked() {
        let mut b = RecordBuilder::new(0);
        assert_eq!(
            b.set(Label::Time, FieldValue::String("1".into())),
            Err(Error::WrongType {
                record: 0,
                label: "t"
            })
        );
        assert_eq!(
            b.set(Label::BaseVersion, FieldValue::Number(5.5)),
            Err(Error::InvalidVersion { record: 0 })
        );
        assert_eq!(
            b.set(Label::Sum, FieldValue::Number(f64::NAN)),
            Err(Error::NonFinite {
                record: 0,
                label: "s".into()
            })
        );
        let mut b = RecordBuilder::new(0);
        assert_eq!(b.set(Label::BaseVersion, FieldValue::Number(5.0)), Ok(()));
        assert_eq!(b.finish().base_version, Some(5));
    }
}
