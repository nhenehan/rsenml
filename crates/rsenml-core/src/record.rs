//! SenML records and packs as decoded, before resolution
//! ([RFC 8428 section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
//! *Resolution* applies the base fields to each record; see `resolve`.
//!
//! Text and binary fields are `Cow`s. A decoder borrows from the input when the
//! format allows it (no escapes in the text). Otherwise it allocates.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use crate::label::Label;

/// The value of a record: one of `v`, `vs`, `vb`, `vd`
/// ([RFC 8428 section 4.2](https://www.rfc-editor.org/rfc/rfc8428#section-4.2)).
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    /// `v`: a number (IEEE double).
    Number(f64),
    /// `vs`: a string.
    String(Cow<'a, str>),
    /// `vb`: a boolean.
    Bool(bool),
    /// `vd`: octets (base64url in JSON and XML, a byte string in CBOR).
    Data(Cow<'a, [u8]>),
}

impl Value<'_> {
    /// The label that carries this value.
    pub fn label(&self) -> Label {
        match self {
            Value::Number(_) => Label::Value,
            Value::String(_) => Label::StringValue,
            Value::Bool(_) => Label::BoolValue,
            Value::Data(_) => Label::DataValue,
        }
    }

    /// The value with its text and data copied out of the input.
    pub fn into_owned(self) -> Value<'static> {
        match self {
            Value::Number(v) => Value::Number(v),
            Value::String(s) => Value::String(Cow::Owned(s.into_owned())),
            Value::Bool(b) => Value::Bool(b),
            Value::Data(d) => Value::Data(Cow::Owned(d.into_owned())),
        }
    }

    /// The number of a `v` value.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(v) => Some(*v),
            _ => None,
        }
    }
}

/// A field with a label this implementation does not know
/// ([RFC 8428 section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4)).
///
/// Readers must ignore unknown labels. rsenml still keeps the ones whose value is a
/// single scalar (number, string, boolean or bytes), which is what the CDDL of
/// [section 11](https://www.rfc-editor.org/rfc/rfc8428#section-11) allows. That way a
/// pack converted from one format to another keeps them. Values that are arrays or
/// maps are skipped. JSON and XML cannot tell a string from base64 data, so an
/// unknown label read from them is always [`Value::String`] (or `Number`/`Bool` in
/// JSON).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Extension<'a> {
    /// The label, as written.
    pub label: Cow<'a, str>,
    /// Its value.
    pub value: Value<'a>,
}

impl<'a> Extension<'a> {
    /// An extension field.
    pub fn new(label: impl Into<Cow<'a, str>>, value: Value<'a>) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }

    /// The field with its text and data copied out of the input.
    pub fn into_owned(self) -> Extension<'static> {
        Extension {
            label: Cow::Owned(self.label.into_owned()),
            value: self.value.into_owned(),
        }
    }

    /// Base extensions (labels that start with `b`,
    /// [section 12.2](https://www.rfc-editor.org/rfc/rfc8428#section-12.2)) apply to
    /// later records.
    pub fn is_base(&self) -> bool {
        crate::label::is_base_label_text(&self.label)
    }
}

/// One SenML record as written, base fields included.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct Record<'a> {
    /// `bn`
    pub base_name: Option<Cow<'a, str>>,
    /// `bt`
    pub base_time: Option<f64>,
    /// `bu`
    pub base_unit: Option<Cow<'a, str>>,
    /// `bv`
    pub base_value: Option<f64>,
    /// `bs`
    pub base_sum: Option<f64>,
    /// `bver`
    pub base_version: Option<u64>,
    /// `n`
    pub name: Option<Cow<'a, str>>,
    /// `u`
    pub unit: Option<Cow<'a, str>>,
    /// `v`, `vs`, `vb` or `vd`
    pub value: Option<Value<'a>>,
    /// `s`
    pub sum: Option<f64>,
    /// `t`
    pub time: Option<f64>,
    /// `ut`
    pub update_time: Option<f64>,
    /// Fields with labels outside RFC 8428, in input order.
    pub extensions: Vec<Extension<'a>>,
}

impl<'a> Record<'a> {
    /// An empty record; add fields with the `with_*` methods.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `bn`.
    pub fn with_base_name(mut self, bn: impl Into<Cow<'a, str>>) -> Self {
        self.base_name = Some(bn.into());
        self
    }

    /// Sets `bt`.
    pub fn with_base_time(mut self, bt: f64) -> Self {
        self.base_time = Some(bt);
        self
    }

    /// Sets `bu`.
    pub fn with_base_unit(mut self, bu: impl Into<Cow<'a, str>>) -> Self {
        self.base_unit = Some(bu.into());
        self
    }

    /// Sets `bv`.
    pub fn with_base_value(mut self, bv: f64) -> Self {
        self.base_value = Some(bv);
        self
    }

    /// Sets `bs`.
    pub fn with_base_sum(mut self, bs: f64) -> Self {
        self.base_sum = Some(bs);
        self
    }

    /// Sets `bver`.
    pub fn with_base_version(mut self, bver: u64) -> Self {
        self.base_version = Some(bver);
        self
    }

    /// Sets `n`.
    pub fn with_name(mut self, n: impl Into<Cow<'a, str>>) -> Self {
        self.name = Some(n.into());
        self
    }

    /// Sets `u`.
    pub fn with_unit(mut self, u: impl Into<Cow<'a, str>>) -> Self {
        self.unit = Some(u.into());
        self
    }

    /// Sets `v` (replacing any other value).
    pub fn with_value(mut self, v: f64) -> Self {
        self.value = Some(Value::Number(v));
        self
    }

    /// Sets `vs` (replacing any other value).
    pub fn with_string_value(mut self, vs: impl Into<Cow<'a, str>>) -> Self {
        self.value = Some(Value::String(vs.into()));
        self
    }

    /// Sets `vb` (replacing any other value).
    pub fn with_bool_value(mut self, vb: bool) -> Self {
        self.value = Some(Value::Bool(vb));
        self
    }

    /// Sets `vd` (replacing any other value).
    pub fn with_data_value(mut self, vd: impl Into<Cow<'a, [u8]>>) -> Self {
        self.value = Some(Value::Data(vd.into()));
        self
    }

    /// Sets `s`.
    pub fn with_sum(mut self, s: f64) -> Self {
        self.sum = Some(s);
        self
    }

    /// Sets `t`.
    pub fn with_time(mut self, t: f64) -> Self {
        self.time = Some(t);
        self
    }

    /// Sets `ut`.
    pub fn with_update_time(mut self, ut: f64) -> Self {
        self.update_time = Some(ut);
        self
    }

    /// Adds a field with a label outside RFC 8428.
    pub fn with_extension(mut self, label: impl Into<Cow<'a, str>>, value: Value<'a>) -> Self {
        self.extensions.push(Extension {
            label: label.into(),
            value,
        });
        self
    }

    /// True when the record carries any base field (registered or extension).
    pub fn has_base_fields(&self) -> bool {
        self.base_name.is_some()
            || self.base_time.is_some()
            || self.base_unit.is_some()
            || self.base_value.is_some()
            || self.base_sum.is_some()
            || self.base_version.is_some()
            || self.extensions.iter().any(Extension::is_base)
    }

    /// True when the record carries any regular (non-base) field.
    pub fn has_regular_fields(&self) -> bool {
        self.name.is_some()
            || self.unit.is_some()
            || self.value.is_some()
            || self.sum.is_some()
            || self.time.is_some()
            || self.update_time.is_some()
            || self.extensions.iter().any(|e| !e.is_base())
    }

    /// A record with only base fields sets values for the records after it. It is
    /// not a measurement itself (see the first record of the example in
    /// [section 5.1.7](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.7)).
    pub fn is_base_only(&self) -> bool {
        !self.has_regular_fields()
    }

    /// Every numeric field, with its label text, for checks that apply to all of them.
    pub fn numbers(&self) -> impl Iterator<Item = (&str, f64)> {
        let fixed = [
            (Label::BaseTime, self.base_time),
            (Label::BaseValue, self.base_value),
            (Label::BaseSum, self.base_sum),
            (Label::Value, self.value.as_ref().and_then(Value::as_number)),
            (Label::Sum, self.sum),
            (Label::Time, self.time),
            (Label::UpdateTime, self.update_time),
        ];
        fixed
            .into_iter()
            .filter_map(|(l, v)| v.map(|v| (l.text(), v)))
            .chain(
                self.extensions
                    .iter()
                    .filter_map(|e| e.value.as_number().map(|v| (&*e.label, v))),
            )
    }

    /// The record with its text and data copied out of the input.
    pub fn into_owned(self) -> Record<'static> {
        Record {
            base_name: self.base_name.map(own_str),
            base_time: self.base_time,
            base_unit: self.base_unit.map(own_str),
            base_value: self.base_value,
            base_sum: self.base_sum,
            base_version: self.base_version,
            name: self.name.map(own_str),
            unit: self.unit.map(own_str),
            value: self.value.map(Value::into_owned),
            sum: self.sum,
            time: self.time,
            update_time: self.update_time,
            extensions: self
                .extensions
                .into_iter()
                .map(Extension::into_owned)
                .collect(),
        }
    }
}

fn own_str(s: Cow<'_, str>) -> Cow<'static, str> {
    Cow::Owned(String::from(s))
}

/// A SenML pack (or a complete SenSML stream): the records in order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pack<'a> {
    /// The records, in order.
    pub records: Vec<Record<'a>>,
}

impl<'a> Pack<'a> {
    /// An empty pack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a record.
    pub fn push(&mut self, record: Record<'a>) {
        self.records.push(record);
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// No records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The records, in order.
    pub fn iter(&self) -> core::slice::Iter<'_, Record<'a>> {
        self.records.iter()
    }

    /// The pack with its text and data copied out of the input.
    pub fn into_owned(self) -> Pack<'static> {
        Pack {
            records: self.records.into_iter().map(Record::into_owned).collect(),
        }
    }
}

impl<'a> From<Vec<Record<'a>>> for Pack<'a> {
    fn from(records: Vec<Record<'a>>) -> Self {
        Self { records }
    }
}

impl<'a> FromIterator<Record<'a>> for Pack<'a> {
    fn from_iter<I: IntoIterator<Item = Record<'a>>>(iter: I) -> Self {
        Self {
            records: iter.into_iter().collect(),
        }
    }
}

impl<'a> IntoIterator for Pack<'a> {
    type Item = Record<'a>;
    type IntoIter = alloc::vec::IntoIter<Record<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.records.into_iter()
    }
}

impl<'p, 'a> IntoIterator for &'p Pack<'a> {
    type Item = &'p Record<'a>;
    type IntoIter = core::slice::Iter<'p, Record<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.records.iter()
    }
}
