//! Number conversions shared by the codecs.
//!
//! SenML numbers are IEEE 754 doubles
//! ([RFC 8428 section 5](https://www.rfc-editor.org/rfc/rfc8428#section-5)). The
//! text formats (JSON, XML) use the shortest digits that read back as the same
//! double. Very large or very small numbers get an exponent with a lower-case `e`
//! (section 5 requires lower case). CBOR and EXI use the most compact exact form:
//! an integer when the value is a whole number; otherwise the smallest float size
//! that holds it exactly (CBOR,
//! [RFC 8428 section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6)), or a
//! decimal mantissa and exponent (EXI,
//! [EXI 7.1.4](https://www.w3.org/TR/exi/#encodingFloat)).
//!
//! The parser and the double builder here do not use `core`'s float parser, to keep
//! small `no_std` builds small. They still round correctly.

use alloc::string::String;
use core::fmt::Write;

/// Appends the shortest text that reads back as `v`. The text is a valid JSON
/// number ([RFC 8259 section 6](https://www.rfc-editor.org/rfc/rfc8259#section-6))
/// and a valid `xs:double` text form (XML Schema calls this the *lexical* form,
/// [XML Schema 2](https://www.w3.org/TR/xmlschema-2/#double)). `v` must be finite.
pub fn write_number(out: &mut String, v: f64) {
    // Writing to a String cannot fail.
    let _ = write_number_to(out, v);
}

/// [`write_number`] into any `core::fmt::Write` (writing to a fixed buffer may
/// fail).
pub fn write_number_to<W: Write>(out: &mut W, v: f64) -> core::fmt::Result {
    let magnitude = v.abs();
    // Both `Display` and `{:e}` print the shortest digits that read back as `v`.
    // `Display` never uses an exponent, so use it only for magnitudes where the
    // plain form is short. Outside that range, `{:e}` writes a lower-case `e`.
    if magnitude == 0.0 || (1e-5..1e16).contains(&magnitude) {
        write!(out, "{v}")
    } else {
        write!(out, "{v:e}")
    }
}

/// [`write_number`] into a new `String`.
pub fn format_number(v: f64) -> String {
    let mut s = String::new();
    write_number(&mut s, v);
    s
}

/// `v` as an `i64` when it is a whole number that an `i64` holds exactly. `-0.0`
/// does not count, because an integer would lose its sign.
pub fn as_exact_i64(v: f64) -> Option<i64> {
    // 2^63: the first magnitude an i64 cannot hold.
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    if !(-LIMIT..LIMIT).contains(&v) {
        return None;
    }
    let i = v as i64;
    (i as f64 == v && !(i == 0 && v.is_sign_negative())).then_some(i)
}

/// The shortest decimal `(mantissa, exponent)` with `mantissa * 10^exponent == v`
/// after rounding to the nearest double. `v` must be finite. This is the EXI Float
/// form ([EXI 7.1.4](https://www.w3.org/TR/exi/#encodingFloat)) of the shortest
/// text form, with no trailing zeros in the mantissa (`20.0` is `(2, 1)`, `120.1`
/// is `(1201, -1)`).
pub fn decimal_parts(v: f64) -> (i64, i32) {
    if v == 0.0 {
        return (0, 0);
    }
    fast_decimal_parts(v).unwrap_or_else(|| shortest_decimal_parts(v))
}

/// A fast path for [`decimal_parts`] that does not use `core::fmt`. It handles
/// whole numbers below 2^53, and values with at most a few decimals whose rounded
/// digits read back as `v` with only one choice. (The shortest-digit formatter
/// took 16-18 % of an EXI encode.) `None`
/// sends the value to [`shortest_decimal_parts`].
///
/// For a whole number below 2^53, doubles are at most 1 apart, so no shorter digit
/// string reads back as it. Its digits (with trailing zeros moved into the
/// exponent) are the shortest form. For other values, try `k` = 0, 1, 2, ...
/// decimals. The first `k` where `round(v * 10^k) * 10^-k` reads back as `v` gives
/// the fewest digits. If the mantissas just above and below also read back, the
/// shortest form would pick the closest one, so that case is left to the general
/// path.
fn fast_decimal_parts(v: f64) -> Option<(i64, i32)> {
    const EXACT: f64 = 9_007_199_254_740_992.0; // 2^53
    let a = v.abs();
    if a >= EXACT {
        return None;
    }
    let reads_back =
        |m: i64, k: i32| decimal_to_f64(i128::from(m), i64::from(-k)).to_bits() == a.to_bits();
    for (k, &p) in POW10.iter().enumerate().take(18) {
        let scaled = a * p;
        if scaled >= EXACT {
            return None;
        }
        // Round half away from zero without `f64::round` (not in core).
        let m = (scaled + 0.5) as i64;
        let k = k as i32;
        if reads_back(m, k) {
            if k > 0 && (reads_back(m - 1, k) || reads_back(m + 1, k)) {
                return None;
            }
            let (mut mantissa, mut exponent) = (m, -k);
            while mantissa != 0 && mantissa % 10 == 0 {
                mantissa /= 10;
                exponent += 1;
            }
            return Some((if v < 0.0 { -mantissa } else { mantissa }, exponent));
        }
    }
    None
}

/// [`decimal_parts`] through `core`'s shortest round-trip formatting (the general
/// path).
fn shortest_decimal_parts(v: f64) -> (i64, i32) {
    // "{:e}" gives the shortest digits that read back as `v`, for example
    // "1.276020076001e9" (at most 17 digits, a point, "e" and a 4-character
    // exponent). It is written to a buffer on the stack, not to an allocated String.
    // Then all the digits become one integer mantissa, and the exponent drops by the
    // number of digits after the point.
    let mut buf = StackText::<32>::new();
    let _ = write!(buf, "{:e}", v.abs());
    let text = buf.as_str();
    let (digits_part, exp_part) = text.split_once('e').unwrap_or((text, "0"));
    let exp: i32 = exp_part.parse().unwrap_or(0);
    let (int_digits, frac_digits) = digits_part.split_once('.').unwrap_or((digits_part, ""));
    let mut mantissa: i64 = 0;
    for b in int_digits.bytes().chain(frac_digits.bytes()) {
        mantissa = mantissa * 10 + i64::from(b - b'0');
    }
    let mut exponent = exp - frac_digits.len() as i32;
    while mantissa != 0 && mantissa % 10 == 0 {
        mantissa /= 10;
        exponent += 1;
    }
    if v < 0.0 {
        mantissa = -mantissa;
    }
    (mantissa, exponent)
}

/// A fixed-size text buffer for `core::fmt` output. When it is full, the write fails
/// and the text stops there.
struct StackText<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> StackText<N> {
    fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(self.buf.get(..self.len).unwrap_or_default()).unwrap_or_default()
    }
}

impl<const N: usize> Write for StackText<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        let dst = self.buf.get_mut(self.len..end).ok_or(core::fmt::Error)?;
        dst.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// The double nearest to `mantissa * 10^exponent` (correctly rounded).
pub fn from_decimal_parts(mantissa: i64, exponent: i32) -> f64 {
    decimal_to_f64(i128::from(mantissa), i64::from(exponent))
}

/// Powers of ten that a double holds exactly (10^22 is the largest one).
const POW10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// The double nearest to `mantissa * 10^exponent` (correctly rounded, ties go to
/// the even neighbour). It does not use `core`'s float parser, whose code and
/// tables take up most of a small `no_std` build.
///
/// When the mantissa fits in 53 bits and the power of ten is exact (|exponent|
/// <= 22), a single IEEE multiply or divide gives the correctly rounded result
/// (Clinger's fast path). That covers what sensors send. Otherwise the value is
/// worked out with a fixed-size big integer and rounded once.
pub fn decimal_to_f64(mantissa: i128, exponent: i64) -> f64 {
    signed_decimal_to_f64(mantissa < 0, mantissa.unsigned_abs(), exponent)
}

/// [`decimal_to_f64`] with the sign kept apart, so that `-0` keeps its sign.
fn signed_decimal_to_f64(negative: bool, m: u128, exponent: i64) -> f64 {
    const EXACT: u128 = 1 << 53;
    let sign = |v: f64| if negative { -v } else { v };
    if m == 0 {
        return sign(0.0);
    }
    if m <= EXACT
        && let Some(&p) = POW10.get(exponent.unsigned_abs() as usize)
    {
        let m = m as f64;
        return sign(if exponent < 0 { m / p } else { m * p });
    }
    let digits = i64::from(decimal_digits(m));
    // m * 10^exponent >= 10^(digits - 1 + exponent), which is beyond f64::MAX
    // (~1.8e308).
    if digits - 1 + exponent > 308 {
        return sign(f64::INFINITY);
    }
    // m * 10^exponent < 10^(digits + exponent) <= 10^-325. That is below half the
    // smallest subnormal (~2.47e-324), so it rounds to zero.
    if digits + exponent <= -325 {
        return sign(0.0);
    }
    if exponent >= 0 {
        // Whole number: compute m * 10^exponent exactly. Keep its top 128 bits;
        // `sticky` records whether any lower bit was set, for correct rounding.
        let mut n = Big::from_u128(m);
        for _ in 0..exponent {
            n.mul_small(10);
        }
        let bits = n.bits();
        let shift = bits.saturating_sub(128);
        let (q, sticky) = (n.extract(shift), n.any_below(shift));
        sign(round_scaled(q, sticky, shift as i32))
    } else {
        // Fraction: divide by 10^-exponent. First shift m left by k bits, so that
        // q = floor(m * 2^k / 10^-exponent) has 126 or 127 significant bits (so it
        // fits a u128). A non-zero remainder sets `sticky`. The result is
        // q * 2^-k.
        let mut d = Big::from_u128(1);
        for _ in 0..-exponent {
            d.mul_small(10);
        }
        let k = (d.bits() + 126).saturating_sub(128 - m.leading_zeros());
        let mut n = Big::from_u128(m);
        n.shl(k);
        let (q, sticky) = n.div_to_u128(&d);
        sign(round_scaled(q, sticky, -(k as i32)))
    }
}

/// Number of decimal digits of `n` (> 0).
fn decimal_digits(mut n: u128) -> u32 {
    let mut d = 0;
    while n > 0 {
        n /= 10;
        d += 1;
    }
    d
}

/// `q * 2^scale`, rounded to the nearest double (ties go to even). `sticky` says
/// the true value is slightly above `q * 2^scale` (some lower bits were cut off).
///
/// It keeps the top 53 bits of `q` (fewer for subnormals), then rounds on the
/// dropped bits: up when they are more than half, and on exactly half, up only if
/// `sticky` is set or the kept value is odd.
fn round_scaled(q: u128, sticky: bool, scale: i32) -> f64 {
    if q == 0 {
        return 0.0;
    }
    let len = 128 - q.leading_zeros() as i32;
    let top = len - 1 + scale; // exponent of the leading bit
    // Bits of precision available at this magnitude (fewer for subnormals).
    let precision = if top >= -1022 { 53 } else { 53 - (-1022 - top) };
    let drop = len - precision;
    let (kept, exp) = if drop <= 0 {
        (q, scale)
    } else {
        if drop > 128 {
            // Far below half the smallest subnormal.
            return 0.0;
        }
        let (kept, rem, half) = if drop == 128 {
            (0u128, q, 1u128 << 127)
        } else {
            (q >> drop, q & ((1u128 << drop) - 1), 1u128 << (drop - 1))
        };
        let up = rem > half || (rem == half && (sticky || kept & 1 == 1));
        (kept + u128::from(up), scale + drop)
    };
    // `kept` has at most 54 bits, and kept * 2^exp is a double (or overflows to
    // infinity). So multiply by exact powers of two. Steps of 2^1000 keep each
    // power of two inside the normal double range.
    let mut v = kept as f64;
    let mut e = exp;
    while e > 1000 {
        v *= pow2(1000);
        e -= 1000;
    }
    while e < -1000 {
        v *= pow2(-1000);
        e += 1000;
    }
    v * pow2(e)
}

/// A fixed-size unsigned big integer: 48 limbs of 32 bits each, low limb first
/// (little-endian). That is enough for the sizes `decimal_to_f64` meets (about 1300
/// bits).
#[derive(Clone)]
struct Big {
    limbs: [u32; 48],
}

impl Big {
    fn from_u128(v: u128) -> Self {
        let mut limbs = [0u32; 48];
        for (i, limb) in limbs.iter_mut().take(4).enumerate() {
            *limb = (v >> (32 * i)) as u32;
        }
        Self { limbs }
    }

    fn mul_small(&mut self, m: u32) {
        let mut carry = 0u64;
        for limb in &mut self.limbs {
            let x = u64::from(*limb) * u64::from(m) + carry;
            *limb = x as u32;
            carry = x >> 32;
        }
    }

    fn bits(&self) -> u32 {
        for (i, &limb) in self.limbs.iter().enumerate().rev() {
            if limb != 0 {
                return 32 * i as u32 + 32 - limb.leading_zeros();
            }
        }
        0
    }

    fn bit(&self, i: u32) -> bool {
        self.limbs
            .get((i / 32) as usize)
            .is_some_and(|l| (l >> (i % 32)) & 1 == 1)
    }

    fn shl(&mut self, n: u32) {
        let (words, bits) = ((n / 32) as usize, n % 32);
        let src = self.limbs;
        for i in (0..self.limbs.len()).rev() {
            let hi = i
                .checked_sub(words)
                .and_then(|j| src.get(j))
                .copied()
                .unwrap_or(0);
            let lo = i
                .checked_sub(words + 1)
                .and_then(|j| src.get(j))
                .copied()
                .unwrap_or(0);
            self.limbs[i] = if bits == 0 {
                hi
            } else {
                (hi << bits) | (lo >> (32 - bits))
            };
        }
    }

    /// Bits `from .. from + 128` as a `u128`.
    fn extract(&self, from: u32) -> u128 {
        (0..128)
            .rev()
            .fold(0u128, |acc, i| (acc << 1) | u128::from(self.bit(from + i)))
    }

    /// Whether any bit below `n` is set.
    fn any_below(&self, n: u32) -> bool {
        (0..n).any(|i| self.bit(i))
    }

    fn ge(&self, other: &Big) -> bool {
        for (a, b) in self.limbs.iter().zip(&other.limbs).rev() {
            if a != b {
                return a > b;
            }
        }
        true
    }

    fn sub(&mut self, other: &Big) {
        let mut borrow = 0i64;
        for (a, &b) in self.limbs.iter_mut().zip(&other.limbs) {
            let x = i64::from(*a) - i64::from(b) - borrow;
            *a = x as u32;
            borrow = i64::from(x < 0);
        }
    }

    /// `(self / d, self % d != 0)` by binary long division. The quotient must fit
    /// in 128 bits.
    fn div_to_u128(&self, d: &Big) -> (u128, bool) {
        let mut r = Big::from_u128(0);
        let mut q = 0u128;
        for i in (0..self.bits()).rev() {
            r.shl(1);
            if self.bit(i) {
                r.limbs[0] |= 1;
            }
            q <<= 1;
            if r.ge(d) {
                r.sub(d);
                q |= 1;
            }
        }
        (q, r.bits() != 0)
    }
}

/// Parses a decimal number in JSON or `xs:double` text form, correctly rounded.
/// The caller has already checked the syntax (`-12.5e+3`, `+.5`, `7`). Up to 37
/// significant digits are kept exactly. Any later non-zero digit is folded in as
/// one "sticky" digit (a 1 added at the end), which marks "a bit more than this"
/// and keeps the rounding exact. 38 digits still fit in a `u128`.
pub fn parse_decimal(text: &str) -> Option<f64> {
    let b = text.as_bytes();
    let mut i = 0;
    let negative = match b.first() {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let mut mantissa: u128 = 0;
    let mut kept = 0u32;
    let mut exponent: i64 = 0;
    let mut sticky = false;
    let mut any_digit = false;
    let mut in_fraction = false;
    while let Some(&c) = b.get(i) {
        match c {
            b'0'..=b'9' => {
                any_digit = true;
                let digit = u128::from(c - b'0');
                if kept < 37 {
                    // Leading zeros are not counted as significant digits.
                    if mantissa != 0 || digit != 0 {
                        mantissa = mantissa * 10 + digit;
                        kept += 1;
                    }
                    if in_fraction {
                        exponent -= 1;
                    }
                } else {
                    // Digit beyond the 37 kept: remember only whether it is
                    // non-zero. In the integer part it still scales the value by 10.
                    sticky |= digit != 0;
                    if !in_fraction {
                        exponent += 1;
                    }
                }
            }
            b'.' if !in_fraction => in_fraction = true,
            b'e' | b'E' => break,
            _ => return None,
        }
        i += 1;
    }
    if !any_digit {
        return None;
    }
    if b.get(i).is_some() {
        i += 1;
        let (exp_negative, start) = match b.get(i) {
            Some(b'-') => (true, i + 1),
            Some(b'+') => (false, i + 1),
            _ => (false, i),
        };
        let digits = b.get(start..)?;
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let e = digits.iter().fold(0i64, |acc, &d| {
            acc.saturating_mul(10).saturating_add(i64::from(d - b'0'))
        });
        exponent = exponent.saturating_add(if exp_negative { -e } else { e });
    }
    if sticky {
        mantissa = mantissa * 10 + 1;
        // The exponent may already sit at i64::MIN (a huge negative exponent).
        exponent = exponent.saturating_sub(1);
    }
    // Clamp huge exponents. Anything this far out is already infinity or zero, and
    // the clamp keeps `digits + exponent` in `signed_decimal_to_f64` from
    // overflowing an i64 (the exponent above saturates at i64::MAX).
    Some(signed_decimal_to_f64(
        negative,
        mantissa,
        exponent.clamp(-100_000, 100_000),
    ))
}

/// `v` as IEEE half-precision (16-bit float) bits when the conversion is exact
/// (`-0.0` included). CBOR uses this for its smallest float size
/// ([RFC 8949 section 3.3](https://www.rfc-editor.org/rfc/rfc8949#section-3.3)).
pub fn to_f16_exact(v: f64) -> Option<u16> {
    let bits = f16_bits_rounded(v)?;
    (f16_to_f64(bits).to_bits() == v.to_bits()).then_some(bits)
}

/// `v` as an `f32` when the conversion is exact.
pub fn to_f32_exact(v: f64) -> Option<f32> {
    let f = v as f32;
    (f64::from(f).to_bits() == v.to_bits()).then_some(f)
}

/// Half-precision bits to a double (always exact). The layout is 1 sign bit, 5
/// exponent bits (bias 15) and 10 fraction bits. This is the decoder of
/// [RFC 8949 Appendix D](https://www.rfc-editor.org/rfc/rfc8949#section-appendix.d).
pub fn f16_to_f64(bits: u16) -> f64 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((bits >> 10) & 0x1f);
    let frac = f64::from(bits & 0x3ff);
    match exp {
        // Exponent 0: subnormal (or zero), value = frac * 2^-24.
        0 => sign * frac * pow2(-24),
        // Exponent 31: infinity when the fraction is 0, otherwise NaN.
        31 if frac == 0.0 => sign * f64::INFINITY,
        31 => f64::NAN,
        _ => sign * (1.0 + frac / 1024.0) * pow2(exp - 15),
    }
}

/// Candidate half-precision bits for a finite `v`. It cuts off extra bits instead
/// of rounding; only exact values pass the round-trip check in [`to_f16_exact`].
fn f16_bits_rounded(v: f64) -> Option<u16> {
    if !v.is_finite() {
        return None;
    }
    let sign: u16 = if v.is_sign_negative() { 0x8000 } else { 0 };
    let a = v.abs();
    if a == 0.0 {
        return Some(sign);
    }
    // Out of range: 65520 is halfway between the largest half (65504) and 2^16, and
    // 2^-24 is the smallest half subnormal.
    if a >= 65520.0 || a < pow2(-24) {
        return None;
    }
    if a < pow2(-14) {
        // Subnormal: value = frac * 2^-24.
        let frac = a / pow2(-24);
        return Some(sign | frac as u16);
    }
    // Normal: find exp with 1 <= a / 2^exp < 2, then keep 10 fraction bits.
    let mut exp = 0i32;
    let mut m = a;
    while m >= 2.0 {
        m /= 2.0;
        exp += 1;
    }
    while m < 1.0 {
        m *= 2.0;
        exp -= 1;
    }
    let frac = ((m - 1.0) * 1024.0) as u16;
    Some(sign | (((exp + 15) as u16) << 10) | frac)
}

// 2^e built straight from the IEEE 754 double bits. Only valid for normal doubles,
// -1022 <= e <= 1023.
fn pow2(e: i32) -> f64 {
    f64::from_bits(((1023 + e) as u64) << 52)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn text_is_shortest_and_lower_case() {
        let cases = [
            (23.1, "23.1"),
            (20.0, "20"),
            (-5.0, "-5"),
            (1.276_020_076_001e9, "1276020076.001"),
            (0.1, "0.1"),
            (1e16, "1e16"),
            (1.5e-7, "1.5e-7"),
            (0.0, "0"),
            (-0.0, "-0"),
            (f64::MAX, "1.7976931348623157e308"),
        ];
        for (v, text) in cases {
            assert_eq!(format_number(v), text);
            assert_eq!(text.parse::<f64>().map(f64::to_bits), Ok(v.to_bits()));
        }
    }

    #[test]
    fn decimal_parts_match_exi_float() {
        assert_eq!(decimal_parts(120.1), (1201, -1));
        assert_eq!(decimal_parts(1.2), (12, -1));
        assert_eq!(decimal_parts(23.1), (231, -1));
        assert_eq!(decimal_parts(20.0), (2, 1));
        assert_eq!(decimal_parts(-0.5), (-5, -1));
        assert_eq!(decimal_parts(1.276_020_076_001e9), (1_276_020_076_001, -3));
        assert_eq!(decimal_parts(0.0), (0, 0));
        for v in [120.1, 1e300, 5e-324, -1.7976931348623157e308, 60.07965] {
            let (m, e) = decimal_parts(v);
            assert_eq!(from_decimal_parts(m, e).to_bits(), v.to_bits());
        }
    }

    /// Random cases per test: full in release builds, a tenth in debug builds.
    const CASES: u32 = if cfg!(debug_assertions) {
        20_000
    } else {
        200_000
    };

    /// xorshift64*, deterministic.
    fn rng(state: &mut u64) -> u64 {
        *state ^= *state >> 12;
        *state ^= *state << 25;
        *state ^= *state >> 27;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    #[test]
    fn decimal_to_f64_matches_core_parser() {
        let mut s = 0x1234_5678_9abc_def1u64;
        for i in 0..CASES {
            let digits = 1 + rng(&mut s) % 38;
            let mut m: i128 = 0;
            for _ in 0..digits {
                m = m * 10 + i128::from(rng(&mut s) % 10);
            }
            if i.is_multiple_of(2) {
                m = -m;
            }
            // Exponents across the whole range, including subnormals and overflow.
            let e = (rng(&mut s) % 720) as i64 - 380;
            let expected: f64 = format!("{m}e{e}").parse().unwrap_or(f64::NAN);
            let got = decimal_to_f64(m, e);
            assert_eq!(got.to_bits(), expected.to_bits(), "{m}e{e}");
        }
    }

    #[test]
    fn halfway_cases_round_to_even() {
        // 2^53 + 1 is halfway between 2^53 and 2^53 + 2: rounds down to even.
        assert_eq!(
            decimal_to_f64(9_007_199_254_740_993, 0),
            9_007_199_254_740_992.0
        );
        assert_eq!(
            decimal_to_f64(9_007_199_254_740_995, 0),
            9_007_199_254_740_996.0
        );
        // Halfway with more digits behind it rounds up.
        assert_eq!(
            parse_decimal("9007199254740993.000000000000000000001"),
            Some(9_007_199_254_740_994.0)
        );
        // Smallest subnormal and the boundary below it.
        assert_eq!(parse_decimal("4.9406564584124654e-324"), Some(5e-324));
        assert_eq!(parse_decimal("2.4703282292062327e-324"), Some(0.0));
        assert_eq!(parse_decimal("2.4703282292062328e-324"), Some(5e-324));
        assert_eq!(parse_decimal("1.7976931348623158e308"), Some(f64::MAX));
        assert_eq!(parse_decimal("1.7976931348623159e308"), Some(f64::INFINITY));
    }

    #[test]
    fn parse_decimal_matches_core_parser() {
        let mut s = 0xdead_beef_cafe_f00du64;
        for _ in 0..CASES / 2 {
            let int_digits = rng(&mut s) % 25;
            let frac_digits = rng(&mut s) % 25;
            let mut t = String::new();
            if rng(&mut s).is_multiple_of(2) {
                t.push('-');
            }
            for _ in 0..int_digits.max(1) {
                t.push(char::from(b'0' + (rng(&mut s) % 10) as u8));
            }
            if frac_digits > 0 {
                t.push('.');
                for _ in 0..frac_digits {
                    t.push(char::from(b'0' + (rng(&mut s) % 10) as u8));
                }
            }
            if rng(&mut s).is_multiple_of(2) {
                t.push('e');
                t.push_str(&format!("{}", (rng(&mut s) % 700) as i64 - 350));
            }
            let expected: f64 = t.parse().unwrap_or(f64::NAN);
            assert_eq!(
                parse_decimal(&t).map(f64::to_bits),
                Some(expected.to_bits()),
                "{t}"
            );
        }
        for t in [
            "0",
            "-0",
            "+.5",
            "1.",
            "007",
            "1E5",
            "0.000001",
            "123456789012345678901234567890123456789012345e-20",
            // More than 37 digits with an exponent that saturates (found by fuzzing).
            "1.000000000000000000000000000000000000001e-99999999999999999999999",
            "1.000000000000000000000000000000000000001e99999999999999999999999",
        ] {
            let expected: f64 = t.parse().unwrap_or(f64::NAN);
            assert_eq!(
                parse_decimal(t).map(f64::to_bits),
                Some(expected.to_bits()),
                "{t}"
            );
        }
        for bad in ["", "-", ".", "e5", "1e", "1e+", "1.2.3", "1x"] {
            assert_eq!(parse_decimal(bad), None, "{bad}");
        }
    }

    #[test]
    fn decimal_to_f64_fast_and_slow_paths() {
        let cases: [(i128, i64, f64); 8] = [
            (2345, -2, 23.45),
            (-5, 0, -5.0),
            (1_276_020_076_001, -3, 1_276_020_076.001),
            (1, 22, 1e22),
            (1, 23, 1e23),
            (9_007_199_254_740_993, 0, 9_007_199_254_740_992.0),
            (17_976_931_348_623_157, 292, f64::MAX),
            (-18_446_744_073_709_551_616, -400, -0.0),
        ];
        for (m, e, v) in cases {
            assert_eq!(decimal_to_f64(m, e).to_bits(), v.to_bits(), "{m}e{e}");
        }
    }

    #[test]
    fn fast_decimal_parts_equals_the_formatter() {
        let mut state = 0x0123_4567_89ab_cdefu64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut values: Vec<f64> = vec![
            0.1,
            0.2,
            0.3,
            1.5,
            23.1,
            120.1,
            1.2,
            60.07965,
            24.30621,
            1_320_067_464.0,
            1_276_020_076.001,
            98.0,
            -0.5,
            1e-7,
            1.5e-7,
            9_007_199_254_740_991.0,
            4_503_599_627_370_497.5,
            0.000_123,
            1e15,
            123_456.789_012,
        ];
        let n = if cfg!(debug_assertions) {
            20_000
        } else {
            2_000_000
        };
        for i in 0..n {
            let r = next();
            values.push(match i % 4 {
                // Arbitrary doubles of every magnitude.
                0 => f64::from_bits(r & !(0x7ff << 52) | ((r % 2046 + 1) << 52)),
                // Sensor-like values: a few decimals.
                1 => {
                    (r % 2_000_000) as f64 / [10.0, 100.0, 1000.0, 1e4][(r >> 40) as usize % 4]
                        - 1000.0
                }
                // Timestamps with milliseconds.
                2 => 1.7e9 + (r % 100_000_000) as f64 / 1000.0,
                // Integers.
                _ => (r >> 12) as f64,
            });
        }
        for v in values {
            if v.is_finite() {
                assert_eq!(decimal_parts(v), shortest_decimal_parts(v), "{v:e}");
            }
        }
    }

    #[test]
    fn exact_integers() {
        assert_eq!(as_exact_i64(-5.0), Some(-5));
        assert_eq!(as_exact_i64(1.5), None);
        assert_eq!(as_exact_i64(-0.0), None);
        assert_eq!(as_exact_i64(1e300), None);
        assert_eq!(as_exact_i64(9_223_372_036_854_775_808.0), None);
    }

    #[test]
    fn half_precision() {
        assert_eq!(to_f16_exact(1.5), Some(0x3e00));
        assert_eq!(to_f16_exact(1.2), None);
        assert_eq!(to_f16_exact(-0.0), Some(0x8000));
        assert_eq!(to_f16_exact(65504.0), Some(0x7bff));
        assert_eq!(to_f16_exact(5.960_464_477_539_063e-8), Some(0x0001));
        assert_eq!(to_f16_exact(100_000.0), None);
        for bits in 0u16..0x7c00 {
            let v = f16_to_f64(bits);
            assert_eq!(to_f16_exact(v), Some(bits), "{bits:#06x}");
        }
        assert_eq!(to_f32_exact(0.1), None);
        assert_eq!(to_f32_exact(0.5), Some(0.5));
    }
}
