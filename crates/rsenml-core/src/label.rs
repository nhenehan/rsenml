//! The SenML labels of RFC 8428 Table 1
//! ([section 4.3](https://www.rfc-editor.org/rfc/rfc8428#section-4.3)), with their
//! JSON/XML names and their CBOR integer keys (Table 4 in
//! [section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)).

/// A label registered by RFC 8428.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Label {
    /// `bn`, Base Name: put in front of the names of this record and later ones.
    BaseName,
    /// `bt`, Base Time: added to the times of this and later records.
    BaseTime,
    /// `bu`, Base Unit: the unit of records that have none.
    BaseUnit,
    /// `bv`, Base Value: added to the numeric values of this and later records.
    BaseValue,
    /// `bs`, Base Sum: added to the sums of this and later records.
    BaseSum,
    /// `bver`, Base Version: the SenML version (10 when absent).
    BaseVersion,
    /// `n`, Name.
    Name,
    /// `u`, Unit.
    Unit,
    /// `v`, numeric Value.
    Value,
    /// `vs`, String Value.
    StringValue,
    /// `vb`, Boolean Value.
    BoolValue,
    /// `vd`, Data Value.
    DataValue,
    /// `s`, Sum: the integral of the value over time.
    Sum,
    /// `t`, Time: absolute when at least 2^28, otherwise relative to "now"
    /// ([section 4.5.3](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.3)).
    Time,
    /// `ut`, Update Time: the longest time to wait before the next reading.
    UpdateTime,
}

/// The JSON/XML value type of a label (Table 2 in
/// [section 5](https://www.rfc-editor.org/rfc/rfc8428#section-5) and Table 5 in
/// [section 7](https://www.rfc-editor.org/rfc/rfc8428#section-7)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LabelType {
    /// A text string.
    String,
    /// JSON Number, XML `double`.
    Number,
    /// JSON Number restricted to positive integers, XML `int` (Base Version).
    Version,
    /// `true` or `false`.
    Boolean,
    /// base64url text in JSON and XML, a byte string in CBOR.
    Data,
}

impl Label {
    /// Every registered label, in the order rsenml writes them. This is the order
    /// the RFC's own examples use, so rsenml writes its JSON and CBOR examples byte
    /// for byte.
    pub const WRITE_ORDER: [Label; 15] = [
        Label::BaseName,
        Label::BaseTime,
        Label::BaseUnit,
        Label::BaseValue,
        Label::BaseSum,
        Label::BaseVersion,
        Label::Name,
        Label::Unit,
        Label::Time,
        Label::UpdateTime,
        Label::Sum,
        Label::Value,
        Label::StringValue,
        Label::BoolValue,
        Label::DataValue,
    ];

    /// The label text, used as the JSON member name and the XML attribute name.
    pub const fn text(self) -> &'static str {
        match self {
            Label::BaseName => "bn",
            Label::BaseTime => "bt",
            Label::BaseUnit => "bu",
            Label::BaseValue => "bv",
            Label::BaseSum => "bs",
            Label::BaseVersion => "bver",
            Label::Name => "n",
            Label::Unit => "u",
            Label::Value => "v",
            Label::StringValue => "vs",
            Label::BoolValue => "vb",
            Label::DataValue => "vd",
            Label::Sum => "s",
            Label::Time => "t",
            Label::UpdateTime => "ut",
        }
    }

    /// The CBOR map key (Table 4 in
    /// [section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)).
    pub const fn cbor(self) -> i8 {
        match self {
            Label::BaseVersion => -1,
            Label::BaseName => -2,
            Label::BaseTime => -3,
            Label::BaseUnit => -4,
            Label::BaseValue => -5,
            Label::BaseSum => -6,
            Label::Name => 0,
            Label::Unit => 1,
            Label::Value => 2,
            Label::StringValue => 3,
            Label::BoolValue => 4,
            Label::Sum => 5,
            Label::Time => 6,
            Label::UpdateTime => 7,
            Label::DataValue => 8,
        }
    }

    /// The type of the label's value (Tables 2 and 5).
    pub const fn value_type(self) -> LabelType {
        match self {
            Label::BaseName | Label::BaseUnit | Label::Name | Label::Unit | Label::StringValue => {
                LabelType::String
            }
            Label::BaseTime
            | Label::BaseValue
            | Label::BaseSum
            | Label::Value
            | Label::Sum
            | Label::Time
            | Label::UpdateTime => LabelType::Number,
            Label::BaseVersion => LabelType::Version,
            Label::BoolValue => LabelType::Boolean,
            Label::DataValue => LabelType::Data,
        }
    }

    /// Base labels
    /// ([section 4.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.1)) apply to
    /// later records too.
    pub const fn is_base(self) -> bool {
        matches!(
            self,
            Label::BaseName
                | Label::BaseTime
                | Label::BaseUnit
                | Label::BaseValue
                | Label::BaseSum
                | Label::BaseVersion
        )
    }

    /// The four value labels. A record carries at most one of them.
    pub const fn is_value(self) -> bool {
        matches!(
            self,
            Label::Value | Label::StringValue | Label::BoolValue | Label::DataValue
        )
    }

    /// The label with this JSON member / XML attribute name.
    pub fn from_text(text: &str) -> Option<Label> {
        Some(match text {
            "bn" => Label::BaseName,
            "bt" => Label::BaseTime,
            "bu" => Label::BaseUnit,
            "bv" => Label::BaseValue,
            "bs" => Label::BaseSum,
            "bver" => Label::BaseVersion,
            "n" => Label::Name,
            "u" => Label::Unit,
            "v" => Label::Value,
            "vs" => Label::StringValue,
            "vb" => Label::BoolValue,
            "vd" => Label::DataValue,
            "s" => Label::Sum,
            "t" => Label::Time,
            "ut" => Label::UpdateTime,
            _ => return None,
        })
    }

    /// The label with this CBOR map key (Table 4 in
    /// [section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)).
    pub fn from_cbor(key: i64) -> Option<Label> {
        Some(match key {
            -1 => Label::BaseVersion,
            -2 => Label::BaseName,
            -3 => Label::BaseTime,
            -4 => Label::BaseUnit,
            -5 => Label::BaseValue,
            -6 => Label::BaseSum,
            0 => Label::Name,
            1 => Label::Unit,
            2 => Label::Value,
            3 => Label::StringValue,
            4 => Label::BoolValue,
            5 => Label::Sum,
            6 => Label::Time,
            7 => Label::UpdateTime,
            8 => Label::DataValue,
            _ => return None,
        })
    }
}

/// [Section 4.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.4): a label that
/// ends in `_` must be understood. A reader that does not know such a label must
/// fail.
pub fn is_must_understand(label: &str) -> bool {
    label.ends_with('_')
}

/// [Section 12.2](https://www.rfc-editor.org/rfc/rfc8428#section-12.2): base labels
/// start with `b`, and regular labels must not.
pub fn is_base_label_text(label: &str) -> bool {
    label.starts_with('b')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_cbor_round_trip() {
        for label in Label::WRITE_ORDER {
            assert_eq!(Label::from_text(label.text()), Some(label));
            assert_eq!(Label::from_cbor(i64::from(label.cbor())), Some(label));
            assert_eq!(label.is_base(), is_base_label_text(label.text()));
        }
        assert_eq!(Label::from_cbor(9), None);
        assert_eq!(Label::from_cbor(-7), None);
    }

    #[test]
    fn write_order_lists_each_label_once() {
        let mut seen = [false; 15];
        for label in Label::WRITE_ORDER {
            let i = (label.cbor() + 6) as usize;
            assert!(!seen[i]);
            seen[i] = true;
        }
    }
}
