#![no_main]
//! The decoders that take input in pieces (JSON and CBOR `StreamDecoder`) and
//! the record-at-a-time readers of every representation. The first byte picks
//! the piece size, so the fuzzer also explores where the input gets cut.
//! When the whole input is a valid pack, the JSON stream decoder must give the
//! same records as the one-piece decode.

use libfuzzer_sys::fuzz_target;
use rsenml::{cbor, exi, json, xml};

fuzz_target!(|data: &[u8]| {
    let Some((&first, input)) = data.split_first() else {
        return;
    };
    let piece = usize::from(first % 16) + 1;

    let mut decoder = json::StreamDecoder::new();
    let mut streamed = Vec::new();
    for chunk in input.chunks(piece) {
        decoder.push(chunk);
        while let Ok(Some(record)) = decoder.next_record() {
            streamed.push(record);
        }
    }
    if decoder.finish().is_ok()
        && let Ok(whole) = json::decode(input)
    {
        assert_eq!(streamed, whole.records, "JSON stream decode differs");
    }

    let mut decoder = cbor::StreamDecoder::new();
    for chunk in input.chunks(piece) {
        decoder.push(chunk);
        while let Ok(Some(_)) = decoder.next_record() {}
    }
    let _ = decoder.finish();

    for record in json::RecordReader::new(input) {
        if record.is_err() {
            break;
        }
    }
    for record in cbor::RecordReader::sensml(input) {
        if record.is_err() {
            break;
        }
    }
    if let Ok(mut reader) = xml::RecordReader::new(input) {
        while let Ok(Some(_)) = reader.next_record() {}
        let _ = reader.finish();
    }
    // Streams without options in the header, read as strict with either alignment.
    for alignment in [exi::Alignment::BitPacked, exi::Alignment::ByteAligned] {
        let options = exi::DecodeOptions::new().with_out_of_band(Some(alignment));
        let _ = exi::decode_with(input, &options);
    }
});
