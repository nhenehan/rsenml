//! A pack of three readings from a room thermostat, written as JSON and read back.

// ANCHOR: main
use rsenml::{Pack, Record};

fn main() -> Result<(), rsenml::Error> {
    // The first record sets a base name, a base time and a base unit. The records
    // after it use them too.
    let pack: Pack = [
        Record::new()
            .with_base_name("urn:dev:mac:0024befffe804ff1:")
            .with_base_time(1_700_000_000.0)
            .with_base_unit("Cel")
            .with_name("room")
            .with_value(21.5),
        Record::new().with_name("setpoint").with_value(21.0),
        Record::new()
            .with_name("heating")
            .with_unit("/")
            .with_value(0.4),
    ]
    .into_iter()
    .collect();

    let json = rsenml::json::encode(&pack)?;
    println!("{}", String::from_utf8_lossy(&json));

    let back = rsenml::json::decode(&json)?;
    assert_eq!(back, pack);
    println!("read back {} records", back.len());
    Ok(())
}
// ANCHOR_END: main
