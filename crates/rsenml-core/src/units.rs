//! The SenML Units registry as RFC 8428 created it (Table 6 in
//! [section 12.1](https://www.rfc-editor.org/rfc/rfc8428#section-12.1)).
//!
//! Later additions are not included: the new units of
//! [RFC 8798 section 2](https://www.rfc-editor.org/rfc/rfc8798#section-2) and its
//! secondary units
//! ([RFC 8798 section 3](https://www.rfc-editor.org/rfc/rfc8798#section-3)). Unit
//! symbols are case-sensitive.

/// One registered unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    /// The symbol used in `u` / `bu`, for example `Cel`.
    pub symbol: &'static str,
    /// What it measures, as Table 6 describes it.
    pub description: &'static str,
    /// Marked `*` in Table 6. New implementations are NOT RECOMMENDED to write it,
    /// but it is in active use and readers SHOULD understand it.
    pub deprecated: bool,
}

const fn u(symbol: &'static str, description: &'static str) -> Unit {
    Unit {
        symbol,
        description,
        deprecated: false,
    }
}

const fn d(symbol: &'static str, description: &'static str) -> Unit {
    Unit {
        symbol,
        description,
        deprecated: true,
    }
}

/// RFC 8428 Table 6, in table order.
pub const UNITS: [Unit; 58] = [
    u("m", "meter"),
    u("kg", "kilogram"),
    d("g", "gram"),
    u("s", "second"),
    u("A", "ampere"),
    u("K", "kelvin"),
    u("cd", "candela"),
    u("mol", "mole"),
    u("Hz", "hertz"),
    u("rad", "radian"),
    u("sr", "steradian"),
    u("N", "newton"),
    u("Pa", "pascal"),
    u("J", "joule"),
    u("W", "watt"),
    u("C", "coulomb"),
    u("V", "volt"),
    u("F", "farad"),
    u("Ohm", "ohm"),
    u("S", "siemens"),
    u("Wb", "weber"),
    u("T", "tesla"),
    u("H", "henry"),
    u("Cel", "degrees Celsius"),
    u("lm", "lumen"),
    u("lx", "lux"),
    u("Bq", "becquerel"),
    u("Gy", "gray"),
    u("Sv", "sievert"),
    u("kat", "katal"),
    u("m2", "square meter (area)"),
    u("m3", "cubic meter (volume)"),
    d("l", "liter (volume)"),
    u("m/s", "meter per second (velocity)"),
    u("m/s2", "meter per square second (acceleration)"),
    u("m3/s", "cubic meter per second (flow rate)"),
    d("l/s", "liter per second (flow rate)"),
    u("W/m2", "watt per square meter (irradiance)"),
    u("cd/m2", "candela per square meter (luminance)"),
    u("bit", "bit (information content)"),
    u("bit/s", "bit per second (data rate)"),
    u("lat", "degrees latitude"),
    u("lon", "degrees longitude"),
    u("pH", "pH value (acidity; logarithmic quantity)"),
    u("dB", "decibel (logarithmic quantity)"),
    u("dBW", "decibel relative to 1 W (power level)"),
    d("Bspl", "bel (sound pressure level; logarithmic quantity)"),
    u("count", "1 (counter value)"),
    u("/", "1 (ratio, e.g., value of a switch)"),
    d("%", "1 (ratio, e.g., value of a switch)"),
    u("%RH", "percentage (relative humidity)"),
    u("%EL", "percentage (remaining battery energy level)"),
    u("EL", "seconds (remaining battery energy level)"),
    u("1/s", "1 per second (event rate)"),
    d("1/min", "1 per minute (event rate, \"rpm\")"),
    d("beat/min", "1 per minute (heart rate in beats per minute)"),
    d("beats", "1 (cumulative number of heart beats)"),
    u("S/m", "siemens per meter (conductivity)"),
];

/// The registered unit with this symbol.
pub fn lookup(symbol: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|unit| unit.symbol == symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_sensitive() {
        assert_eq!(lookup("Cel").map(|u| u.deprecated), Some(false));
        assert_eq!(lookup("cel"), None);
        assert_eq!(lookup("%").map(|u| u.deprecated), Some(true));
        assert_eq!(lookup("furlong"), None);
    }

    #[test]
    fn symbols_are_unique() {
        for (i, a) in UNITS.iter().enumerate() {
            assert!(
                UNITS[i + 1..].iter().all(|b| b.symbol != a.symbol),
                "{}",
                a.symbol
            );
        }
    }
}
