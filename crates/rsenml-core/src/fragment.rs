//! Record fragment identifiers
//! ([RFC 8428 section 9](https://www.rfc-editor.org/rfc/rfc8428#section-9)):
//! `rec=3`, `rec=3-6`, `rec=19-*`, `rec=3-5,10,19-*`. Positions start at 1.
//!
//! A selected record only keeps its meaning together with the base values of the
//! records before it. So selection works on whole packs and returns positions. To
//! use the selected records on their own, resolve the whole pack first. A range
//! whose end comes before its start is rejected.

use alloc::vec::Vec;
use core::fmt;

/// One item of a `rec=` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordRange {
    /// `n`
    Single(usize),
    /// `n-m`
    Range(usize, usize),
    /// `n-*`: from `n` to the last record.
    ToEnd(usize),
}

impl RecordRange {
    /// Whether the 1-based `position` is selected in a pack of `len` records.
    pub fn contains(self, position: usize, len: usize) -> bool {
        position >= 1
            && position <= len
            && match self {
                RecordRange::Single(n) => position == n,
                RecordRange::Range(a, b) => (a..=b).contains(&position),
                RecordRange::ToEnd(a) => position >= a,
            }
    }
}

/// A parsed `rec=` fragment identifier
/// ([section 9.1](https://www.rfc-editor.org/rfc/rfc8428#section-9.1) has examples).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSelector {
    /// The comma-separated items, in order.
    pub ranges: Vec<RecordRange>,
}

/// The fragment is not a valid `rec=` selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidFragment;

impl fmt::Display for InvalidFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a SenML rec= fragment identifier")
    }
}

impl core::error::Error for InvalidFragment {}

impl RecordSelector {
    /// Parses `rec=...`, with or without a leading `#`.
    pub fn parse(fragment: &str) -> Result<Self, InvalidFragment> {
        let fragment = fragment.strip_prefix('#').unwrap_or(fragment);
        let list = fragment.strip_prefix("rec=").ok_or(InvalidFragment)?;
        let mut ranges = Vec::new();
        for item in list.split(',') {
            ranges.push(match item.split_once('-') {
                None => RecordRange::Single(position(item)?),
                Some((a, "*")) => RecordRange::ToEnd(position(a)?),
                Some((a, b)) => {
                    let (a, b) = (position(a)?, position(b)?);
                    if b < a {
                        return Err(InvalidFragment);
                    }
                    RecordRange::Range(a, b)
                }
            });
        }
        Ok(Self { ranges })
    }

    /// Whether the 1-based `position` is selected in a pack of `len` records.
    pub fn contains(&self, position: usize, len: usize) -> bool {
        self.ranges.iter().any(|r| r.contains(position, len))
    }

    /// The selected 0-based indices of a pack of `len` records, ascending.
    pub fn indices(&self, len: usize) -> impl Iterator<Item = usize> + '_ {
        (0..len).filter(move |&i| self.contains(i + 1, len))
    }
}

// A record position: one or more ASCII digits, not zero. The digit check comes
// first because `parse` would also accept a leading `+`.
fn position(text: &str) -> Result<usize, InvalidFragment> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(InvalidFragment);
    }
    match text.parse() {
        Ok(0) | Err(_) => Err(InvalidFragment),
        Ok(n) => Ok(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn selected(fragment: &str, len: usize) -> Vec<usize> {
        RecordSelector::parse(fragment)
            .map(|s| s.indices(len).map(|i| i + 1).collect())
            .unwrap_or_default()
    }

    #[test]
    fn rfc8428_section_9_1() {
        assert_eq!(selected("rec=3", 25), vec![3]);
        assert_eq!(selected("rec=3-6", 25), vec![3, 4, 5, 6]);
        assert_eq!(selected("rec=19-*", 21), vec![19, 20, 21]);
        assert_eq!(selected("rec=3,5", 25), vec![3, 5]);
        assert_eq!(selected("#rec=3-5,10,19-*", 20), vec![3, 4, 5, 10, 19, 20]);
    }

    #[test]
    fn invalid() {
        for f in [
            "rec=", "rec=0", "rec=a", "rec=5-3", "row=1", "rec=1,", "rec=-3", "rec=+1",
        ] {
            assert_eq!(RecordSelector::parse(f), Err(InvalidFragment), "{f}");
        }
        assert_eq!(selected("rec=30", 25), Vec::<usize>::new());
    }
}
