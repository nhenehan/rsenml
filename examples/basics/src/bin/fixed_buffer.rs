//! A soil moisture node with no heap to spare writes its pack into a fixed buffer.

// ANCHOR: main
use rsenml::exi::EncodeOptions;
use rsenml::{Pack, Record};

fn main() -> Result<(), rsenml::Error> {
    let pack: Pack = [
        Record::new()
            .with_base_name("urn:dev:mac:0024befffe804ff5:")
            .with_name("moisture")
            .with_unit("%RH")
            .with_value(31.0),
        Record::new()
            .with_name("soil-temp")
            .with_unit("Cel")
            .with_value(14.5),
    ]
    .into_iter()
    .collect();

    // The output goes into this buffer, not into a growing Vec.
    let mut buf = [0u8; 128];
    let n = rsenml::cbor::encode_into(&pack, &mut buf)?;
    println!("CBOR: {n} bytes");
    let n = rsenml::exi::encode_into(&pack, &mut buf, &EncodeOptions::new())?;
    println!("EXI:  {n} bytes");

    // When the buffer is too small, encoding fails instead of writing part of a
    // pack.
    let mut small = [0u8; 8];
    println!(
        "too small: {:?}",
        rsenml::cbor::encode_into(&pack, &mut small)
    );
    Ok(())
}
// ANCHOR_END: main
