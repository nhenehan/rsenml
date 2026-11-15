//! Checking packs from an HVAC unit against the RFC's rules.

// ANCHOR: main
use rsenml::{Pack, Record, ValidationOptions, Value};

fn main() {
    let good: Pack = [
        Record::new()
            .with_base_name("urn:dev:mac:0024befffe804ff3:")
            .with_name("supply-air")
            .with_unit("Cel")
            .with_value(12.8),
        Record::new().with_name("compressor").with_bool_value(true),
    ]
    .into_iter()
    .collect();
    println!("good pack: {:?}", good.validate());

    // A record needs a value (or a sum).
    let no_value: Pack = [Record::new()
        .with_base_name("urn:dev:mac:0024befffe804ff3:")
        .with_name("fan")]
    .into_iter()
    .collect();
    println!("no value: {:?}", no_value.validate());

    // Labels ending in "_" must be understood, so an unknown one is refused.
    let must_understand: Pack = [Record::new()
        .with_base_name("urn:dev:mac:0024befffe804ff3:")
        .with_name("fan")
        .with_value(0.65)
        .with_extension("mode_", Value::String("eco".into()))]
    .into_iter()
    .collect();
    println!("unknown _ label: {:?}", must_understand.validate());

    // "RPM" is not in the SenML units registry. RFC 8428 allows it, unless you ask
    // for registered units only.
    let rpm: Pack = [Record::new()
        .with_base_name("urn:dev:mac:0024befffe804ff3:")
        .with_name("fan-speed")
        .with_unit("RPM")
        .with_value(900.0)]
    .into_iter()
    .collect();
    println!("RPM: {:?}", rpm.validate());
    let strict = ValidationOptions::new().with_registered_units(true);
    println!(
        "RPM, registered units only: {:?}",
        rpm.validate_with(&strict)
    );
}
// ANCHOR_END: main
