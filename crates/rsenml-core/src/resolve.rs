//! Resolved records
//! ([RFC 8428 section 4.6](https://www.rfc-editor.org/rfc/rfc8428#section-4.6)). A
//! *resolved record* has every base value applied to it and an absolute time, so it
//! makes sense on its own, outside its pack.
//!
//! How rsenml reads the spec where it leaves room (also listed in `doc/design.md`):
//! - `bv` is added to a numeric `v` only. A record with no value field gets no
//!   value (it is a sum-only record), even when a `bv` is in force.
//! - The sum is `bs + s` when either one is present
//!   ([section 4.5.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.4)). So a
//!   `bs` alone gives every later record a sum.
//! - Times below 2^28 are relative to "now"
//!   ([section 4.5.3](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.3)). The
//!   caller gives "now". Resolving such a time without it is an error.
//! - Unknown base extensions cannot be resolved (section 4.6 leaves that to the spec
//!   that defines them), so they are dropped. Regular extensions are kept.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{Error, Result};
use crate::record::{Extension, Pack, Record, Value};
use crate::validate::SUPPORTED_VERSION;

/// 2^28 (268,435,456). Times at or above this are absolute (seconds since the Unix
/// epoch, so 2^28 itself is 1978-07-04 21:24:16 UTC). Times below it are relative
/// to "now"
/// ([section 4.5.3](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.3)).
pub const ABSOLUTE_TIME_THRESHOLD: f64 = 268_435_456.0;

/// True when `t` is a relative time.
pub fn is_relative_time(t: f64) -> bool {
    t < ABSOLUTE_TIME_THRESHOLD
}

/// A record with every base value applied and an absolute time.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ResolvedRecord<'a> {
    /// Base name + name.
    pub name: String,
    /// Unit, or the base unit in force.
    pub unit: Option<Cow<'a, str>>,
    /// Value, numeric ones with the base value added.
    pub value: Option<Value<'a>>,
    /// Base sum + sum, when either is present.
    pub sum: Option<f64>,
    /// Absolute time, seconds since the Unix epoch.
    pub time: f64,
    /// Update time, unchanged.
    pub update_time: Option<f64>,
    /// The pack's version, when it is not 10. Section 4.6 says a resolved record
    /// leaves out `bver` for version 10 and must carry it for any other version
    /// ([section 4.6](https://www.rfc-editor.org/rfc/rfc8428#section-4.6)).
    pub version: Option<u64>,
    /// Regular (non-base) extensions.
    pub extensions: Vec<Extension<'a>>,
}

impl<'a> ResolvedRecord<'a> {
    /// The resolved record as a plain record (`bver` carries the version when it is
    /// not 10). This lets a resolved pack be written in any format.
    pub fn into_record(self) -> Record<'a> {
        Record {
            base_version: self.version,
            name: Some(Cow::Owned(self.name)),
            unit: self.unit,
            value: self.value,
            sum: self.sum,
            time: Some(self.time),
            update_time: self.update_time,
            extensions: self.extensions,
            ..Record::default()
        }
    }
}

/// Applies base values one record at a time. It works the same for packs and for
/// SenSML streams (records that arrive over time,
/// [section 4.8](https://www.rfc-editor.org/rfc/rfc8428#section-4.8)).
///
/// It keeps the base values in force. Each base field stays in force until a later
/// record sets the same field again
/// ([section 4](https://www.rfc-editor.org/rfc/rfc8428#section-4)).
#[derive(Debug, Clone)]
pub struct Resolver<'a> {
    now: Option<f64>,
    index: usize,
    base_name: Option<Cow<'a, str>>,
    base_time: Option<f64>,
    base_unit: Option<Cow<'a, str>>,
    base_value: Option<f64>,
    base_sum: Option<f64>,
    version: u64,
}

impl<'a> Resolver<'a> {
    /// `now` is used to resolve relative times. It is the time the pack was sent (or,
    /// for a SenSML stream, the time the record was sent), in seconds since the Unix
    /// epoch.
    pub fn new(now: Option<f64>) -> Self {
        Self {
            now,
            index: 0,
            base_name: None,
            base_time: None,
            base_unit: None,
            base_value: None,
            base_sum: None,
            version: SUPPORTED_VERSION,
        }
    }

    /// Changes "now" for the records that follow. In a SenSML stream, "now" is set
    /// for each record
    /// ([section 4.8](https://www.rfc-editor.org/rfc/rfc8428#section-4.8)).
    pub fn set_now(&mut self, now: Option<f64>) {
        self.now = now;
    }

    /// Applies `record`'s base fields, then resolves it. Returns `None` for a record
    /// with only base fields. The record should have passed validation.
    ///
    /// The steps follow
    /// [section 4.6](https://www.rfc-editor.org/rfc/rfc8428#section-4.6):
    /// name = `bn` + `n`; unit = `u`, or else `bu`; value = `bv` + `v`
    /// ([4.5.4](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.4)); time =
    /// `bt` + `t`, plus "now" when the total is relative
    /// ([4.5.3](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.3)).
    pub fn resolve(&mut self, record: &Record<'a>) -> Result<Option<ResolvedRecord<'a>>> {
        let index = self.index;
        self.index += 1;
        // A base field applies to the record that holds it, as well as to later
        // records. So update the base values first.
        if let Some(bn) = &record.base_name {
            self.base_name = Some(bn.clone());
        }
        if let Some(bt) = record.base_time {
            self.base_time = Some(bt);
        }
        if let Some(bu) = &record.base_unit {
            self.base_unit = Some(bu.clone());
        }
        if let Some(bv) = record.base_value {
            self.base_value = Some(bv);
        }
        if let Some(bs) = record.base_sum {
            self.base_sum = Some(bs);
        }
        if let Some(bver) = record.base_version {
            self.version = bver;
        }
        if record.is_base_only() {
            return Ok(None);
        }

        let mut name = String::new();
        if let Some(bn) = &self.base_name {
            name.push_str(bn);
        }
        if let Some(n) = &record.name {
            name.push_str(n);
        }
        let value = record.value.as_ref().map(|v| match v {
            Value::Number(v) => Value::Number(self.base_value.unwrap_or(0.0) + v),
            other => other.clone(),
        });
        let sum = match (self.base_sum, record.sum) {
            (None, None) => None,
            (bs, s) => Some(bs.unwrap_or(0.0) + s.unwrap_or(0.0)),
        };
        // A missing `bt` or `t` counts as zero. The test for "relative" is on the
        // total, since the spec warns that `bt + t` can cross 2^28.
        let mut time = self.base_time.unwrap_or(0.0) + record.time.unwrap_or(0.0);
        if is_relative_time(time) {
            let now = self.now.ok_or(Error::RelativeTime { record: index })?;
            time += now;
        }
        Ok(Some(ResolvedRecord {
            name,
            unit: record.unit.clone().or_else(|| self.base_unit.clone()),
            value,
            sum,
            time,
            update_time: record.update_time,
            version: (self.version != SUPPORTED_VERSION).then_some(self.version),
            extensions: record
                .extensions
                .iter()
                .filter(|e| !e.is_base())
                .cloned()
                .collect(),
        }))
    }
}

impl<'a> Pack<'a> {
    /// Validates the pack, resolves every record and sorts them by time, as
    /// [section 4.6](https://www.rfc-editor.org/rfc/rfc8428#section-4.6) asks. The
    /// sort is stable: records with equal times keep their order.
    pub fn resolve(&self, now: Option<f64>) -> Result<Vec<ResolvedRecord<'a>>> {
        self.validate()?;
        let mut resolver = Resolver::new(now);
        let mut out = Vec::with_capacity(self.len());
        for record in self {
            if let Some(r) = resolver.resolve(record)? {
                out.push(r);
            }
        }
        out.sort_by(|a, b| a.time.total_cmp(&b.time));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn rfc8428_section_5_1_4() -> Result<()> {
        // Section 5.1.3, resolved in section 5.1.4.
        let rec = |u: Option<&'static str>, t: Option<f64>, v: f64| {
            let mut r = Record::new().with_value(v);
            r.unit = u.map(Cow::Borrowed);
            r.time = t;
            r
        };
        let mut first = rec(None, None, 20.0)
            .with_base_name("urn:dev:ow:10e2073a01080063")
            .with_base_unit("%RH");
        first.base_time = Some(1.320067464e9);
        let pack = Pack::from(vec![
            first,
            rec(Some("lon"), None, 24.30621),
            rec(Some("lat"), None, 60.07965),
            rec(None, Some(60.0), 20.3),
            rec(Some("%EL"), Some(150.0), 98.0),
        ]);
        let resolved = pack.resolve(None)?;
        let names: Vec<_> = resolved.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["urn:dev:ow:10e2073a01080063"; 5]);
        let units: Vec<_> = resolved.iter().map(|r| r.unit.as_deref()).collect();
        assert_eq!(
            units,
            [
                Some("%RH"),
                Some("lon"),
                Some("lat"),
                Some("%RH"),
                Some("%EL")
            ]
        );
        let times: Vec<_> = resolved.iter().map(|r| r.time).collect();
        assert_eq!(
            times,
            [
                1.320067464e9,
                1.320067464e9,
                1.320067464e9,
                1.320067524e9,
                1.320067614e9
            ]
        );
        assert!(resolved.iter().all(|r| r.version.is_none()));
        Ok(())
    }

    #[test]
    fn base_value_sum_relative_time_and_order() -> Result<()> {
        let pack = Pack::from(vec![
            Record::new()
                .with_base_name("dev/")
                .with_base_value(100.0)
                .with_base_sum(10.0)
                .with_base_version(5)
                .with_name("a")
                .with_value(1.0)
                .with_time(-1.0),
            Record::new()
                .with_name("b")
                .with_string_value("x")
                .with_time(-2.0),
        ]);
        assert_eq!(pack.resolve(None), Err(Error::RelativeTime { record: 0 }));
        let resolved = pack.resolve(Some(1000.0))?;
        assert_eq!(resolved[0].name, "dev/b");
        assert_eq!(resolved[0].time, 998.0);
        assert_eq!(resolved[0].value, Some(Value::String("x".into())));
        assert_eq!(resolved[0].sum, Some(10.0));
        assert_eq!(resolved[1].value, Some(Value::Number(101.0)));
        assert_eq!(resolved[1].version, Some(5));
        let rec = resolved[1].clone().into_record();
        assert_eq!(rec.base_version, Some(5));
        assert_eq!(rec.name.as_deref(), Some("dev/a"));
        Ok(())
    }
}
