//! A tank level sensor sends SenSML: records go out one at a time, and the reader
//! uses each one as soon as its bytes arrive.

// ANCHOR: main
use rsenml::Record;
use rsenml::json::{StreamDecoder, StreamEncoder};

fn main() -> Result<(), rsenml::Error> {
    // The sensor writes one record each minute into the same stream.
    let mut wire = Vec::new();
    let mut encoder = StreamEncoder::new();
    encoder.record(
        &Record::new()
            .with_base_name("urn:dev:mac:0024befffe804ff4:tank:")
            .with_base_time(1_700_000_000.0)
            .with_name("level")
            .with_unit("m")
            .with_value(1.82),
        &mut wire,
    )?;
    for (minute, level) in [(1.0, 1.81), (2.0, 1.79)] {
        let record = Record::new()
            .with_name("level")
            .with_time(minute * 60.0)
            .with_value(level);
        encoder.record(&record, &mut wire)?;
    }
    encoder.finish(&mut wire);
    println!("{}", String::from_utf8_lossy(&wire));

    // The reader gets the bytes in pieces of 16, as a network might deliver them.
    let mut decoder = StreamDecoder::new();
    for piece in wire.chunks(16) {
        decoder.push(piece);
        while let Some(record) = decoder.next_record()? {
            println!("got {:?} = {:?}", record.name, record.value);
        }
    }
    decoder.finish()?;
    println!("stream finished: {}", decoder.is_finished());
    Ok(())
}
// ANCHOR_END: main
