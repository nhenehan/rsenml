//! Readings from a home water meter: base fields, relative times and resolving.

// ANCHOR: main
use rsenml::{Pack, Record};

fn main() -> Result<(), rsenml::Error> {
    // The meter sends its total (a sum, in litres) every 10 minutes. The times are
    // relative to the base time: -1200 s, -600 s and 0 s.
    let pack: Pack = [
        Record::new()
            .with_base_name("urn:dev:mac:0024befffe804ff2:water:")
            .with_base_time(1_700_000_000.0)
            .with_base_unit("l")
            .with_base_sum(48_000.0)
            .with_name("total")
            .with_time(-1200.0)
            .with_sum(12.5),
        Record::new()
            .with_name("total")
            .with_time(-600.0)
            .with_sum(19.0),
        Record::new().with_name("total").with_sum(31.5),
    ]
    .into_iter()
    .collect();

    // Resolving gives every record its full name, unit and time, and adds the base
    // sum. "Now" is only needed for times below 2^28, which this pack does not use.
    for r in pack.resolve(None)? {
        println!(
            "{} t={} u={:?} sum={:?} value={:?}",
            r.name, r.time, r.unit, r.sum, r.value
        );
    }
    Ok(())
}
// ANCHOR_END: main
