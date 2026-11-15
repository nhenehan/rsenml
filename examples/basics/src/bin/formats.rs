//! The thermostat pack in all eight SenML media types, with their sizes.

// ANCHOR: main
use rsenml::exi::{Alignment, EncodeOptions};
use rsenml::{MediaType, Pack, Record};

fn main() -> Result<(), rsenml::Error> {
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

    for media in MediaType::ALL {
        let bytes = rsenml::encode(media, &pack)?;
        assert_eq!(rsenml::decode(media, &bytes)?, pack);
        println!(
            "{:28} CoAP {:>3}: {:>3} bytes",
            media.content_type(),
            media.coap_content_format(),
            bytes.len()
        );
    }

    // EXI is bit-packed by default. Byte-aligned EXI is a little bigger, but every
    // value starts on a byte boundary.
    let options = EncodeOptions::new().with_alignment(Alignment::ByteAligned);
    let aligned = rsenml::exi::encode_with(&pack, &options)?;
    println!("EXI byte-aligned: {} bytes", aligned.len());

    // A content type from an HTTP header or a CoAP option picks the format.
    let media = MediaType::from_content_type("application/senml+cbor");
    println!("{media:?}");
    Ok(())
}
// ANCHOR_END: main
