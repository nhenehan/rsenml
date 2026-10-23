//! SenML CBOR reading and writing.

use rsenml_cbor::{
    RecordReader, StreamDecoder, StreamEncoder, decode, decode_sensml, decode_unchecked, encode,
    encode_into, encode_sensml,
};
use rsenml_core::{Error, Format, Pack, Record, Result, Value};

/// RFC 8428 section 6, the hex dump (0xc3 bytes).
const RFC_SECTION_6: &str = "\
87 a7 21 78 1b 75 72 6e 3a 64 65 76 3a 6f 77 3a
31 30 65 32 30 37 33 61 30 31 30 38 30 30 36 3a
22 fb 41 d3 03 a1 5b 00 10 62 23 61 41 20 05 00
67 76 6f 6c 74 61 67 65 01 61 56 02 fb 40 5e 06
66 66 66 66 66 a3 00 67 63 75 72 72 65 6e 74 06
24 02 fb 3f f3 33 33 33 33 33 33 a3 00 67 63 75
72 72 65 6e 74 06 23 02 fb 3f f4 cc cc cc cc cc
cd a3 00 67 63 75 72 72 65 6e 74 06 22 02 fb 3f
f6 66 66 66 66 66 66 a3 00 67 63 75 72 72 65 6e
74 06 21 02 f9 3e 00 a3 00 67 63 75 72 72 65 6e
74 06 20 02 fb 3f f9 99 99 99 99 99 9a a3 00 67
63 75 72 72 65 6e 74 06 00 02 fb 3f fb 33 33 33
33 33 33";

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .filter_map(|b| u8::from_str_radix(b, 16).ok())
        .collect()
}

/// The section 6 example in diagnostic notation, as a pack.
fn rfc_pack() -> Pack<'static> {
    let mut records = vec![
        Record::new()
            .with_base_name("urn:dev:ow:10e2073a0108006:")
            .with_base_time(1_276_020_076.001)
            .with_base_unit("A")
            .with_base_version(5)
            .with_name("voltage")
            .with_unit("V")
            .with_value(120.1),
    ];
    for (t, v) in [
        (-5.0, 1.2),
        (-4.0, 1.3),
        (-3.0, 1.4),
        (-2.0, 1.5),
        (-1.0, 1.6),
        (0.0, 1.7),
    ] {
        records.push(
            Record::new()
                .with_name("current")
                .with_time(t)
                .with_value(v),
        );
    }
    Pack::from(records)
}

#[test]
fn rfc8428_section_6_both_ways() -> Result<()> {
    let bytes = hex(RFC_SECTION_6);
    assert_eq!(bytes.len(), 0xc3);
    assert_eq!(decode(&bytes)?, rfc_pack());
    assert_eq!(encode(&rfc_pack())?, bytes);
    Ok(())
}

#[test]
fn strings_borrow_from_the_input() -> Result<()> {
    let bytes = hex(RFC_SECTION_6);
    let pack = decode(&bytes)?;
    assert!(matches!(
        pack.records[0].base_name,
        Some(std::borrow::Cow::Borrowed(_))
    ));
    Ok(())
}

#[test]
fn numbers_take_the_smallest_exact_form() -> Result<()> {
    let pack = Pack::from(vec![
        Record::new()
            .with_name("a")
            .with_value(1.5)
            .with_time(1_700_000_000.0)
            .with_sum(0.1)
            .with_update_time(0.5),
    ]);
    let bytes = encode(&pack)?;
    // map(4): 0:"a", 6:1700000000 (u32), 7:0.5 (f16), 5:0.1 (f64), 2:1.5 (f16)
    assert_eq!(
        bytes,
        hex(
            "81 a5 00 61 61 06 1a 65 53 f1 00 07 f9 38 00 05 fb 3f b9 99 99 99 99 99 9a 02 f9 3e 00"
        )
    );
    assert_eq!(decode(&bytes)?, pack);
    // f32 when exact and not representable as half.
    let pack = Pack::from(vec![Record::new().with_name("a").with_value(100_000.5)]);
    assert_eq!(encode(&pack)?, hex("81 a2 00 61 61 02 fa 47 c3 50 40"));
    Ok(())
}

#[test]
fn every_numeric_form_is_read() -> Result<()> {
    // v as u8, negative int, f16, f32, f64 and tag 4 decimal fraction 4(-2, 2345) = 23.45
    for (value, expected) in [
        ("18 17", 23.0),
        ("38 16", -23.0),
        ("f9 3e 00", 1.5),
        ("fa 41 b8 cc cd", f64::from(23.1f32)),
        ("fb 40 37 19 99 99 99 99 9a", 23.1),
        ("c4 82 21 19 09 29", 23.45),
        ("1b 00 00 00 01 00 00 00 00", 4_294_967_296.0),
    ] {
        let bytes = hex(&format!("81 a2 00 61 61 02 {value}"));
        let pack = decode(&bytes)?;
        assert_eq!(
            pack.records[0].value,
            Some(Value::Number(expected)),
            "{value}"
        );
    }
    Ok(())
}

#[test]
fn text_keys_spelling_registered_labels_are_ignored() -> Result<()> {
    // {0: "a", 2: 1, "v": 2, "bn": "x"}: the integer key 2 is the value; the
    // text keys are unrecognized fields (RFC 8428 4.4) and are dropped.
    let bytes = hex("81 a4 00 61 61 02 01 61 76 02 62 62 6e 61 78");
    let pack = decode(&bytes)?;
    assert_eq!(pack.records[0].value, Some(Value::Number(1.0)));
    assert_eq!(pack.records[0].base_name, None);
    assert!(pack.records[0].extensions.is_empty());
    Ok(())
}

#[test]
fn record_rules() {
    let bad = |h: &str| decode(&hex(h)).map(Pack::into_owned);
    // text key "v" is not Value in CBOR: ignored, so this record has no value
    assert_eq!(
        bad("81 a2 00 61 61 61 76 01"),
        Err(Error::MissingValue { record: 0 })
    );
    // must-understand text label
    assert!(matches!(
        bad("81 a3 00 61 61 02 01 62 78 5f f5"),
        Err(Error::MustUnderstand { .. })
    ));
    // bver as a float is not allowed (section 6)
    assert_eq!(
        bad("81 a3 00 61 61 02 01 20 f9 45 00"),
        Err(Error::WrongType {
            record: 0,
            label: "bver"
        })
    );
    // duplicate key
    assert!(matches!(
        bad("81 a3 00 61 61 00 61 62 02 01"),
        Err(Error::DuplicateLabel { .. })
    ));
    // two value labels
    assert_eq!(
        bad("81 a3 00 61 61 02 01 04 f5"),
        Err(Error::MultipleValues { record: 0 })
    );
    // vs as bytes
    assert!(matches!(
        bad("81 a2 00 61 61 03 41 00"),
        Err(Error::WrongType { .. })
    ));
    // NaN
    assert!(matches!(
        bad("81 a2 00 61 61 02 f9 7e 00"),
        Err(Error::NonFinite { .. })
    ));
    // SenML must be a definite array
    assert!(matches!(
        bad("9f a2 00 61 61 02 01 ff"),
        Err(Error::Syntax { .. })
    ));
    // trailing bytes
    assert!(matches!(
        bad("81 a2 00 61 61 02 01 00"),
        Err(Error::Syntax { .. })
    ));
    assert_eq!(
        bad("81 a2 00 61 61 02"),
        Err(Error::Incomplete {
            format: Format::Cbor
        })
    );
}

#[test]
fn unknown_integer_keys_are_ignored_and_text_extensions_kept() -> Result<()> {
    // key 9 (not in Table 4) and key -7 ignored; "foo": h'01', "bar": [1] skipped
    let bytes = hex("81 a6 00 61 61 02 01 09 f5 26 61 78 63 66 6f 6f 41 01 63 62 61 72 81 01");
    let pack = decode_unchecked(&bytes)?;
    let exts: Vec<_> = pack.records[0]
        .extensions
        .iter()
        .map(|e| (e.label.as_ref(), e.value.clone()))
        .collect();
    assert_eq!(exts, [("foo", Value::Data(vec![1].into()))]);
    Ok(())
}

#[test]
fn large_packs_and_integer_times() -> Result<()> {
    let pack: Pack = (0..100)
        .map(|i| {
            Record::new()
                .with_name("sensor")
                .with_time(1_700_000_000.0 + f64::from(i))
                .with_value(f64::from(i))
        })
        .collect();
    let bytes = encode(&pack)?;
    assert_eq!(decode(&bytes)?, pack);
    Ok(())
}

#[test]
fn sensml_uses_an_indefinite_array() -> Result<()> {
    let pack = rfc_pack();
    let bytes = encode_sensml(&pack)?;
    assert_eq!(bytes.first(), Some(&0x9f));
    assert_eq!(bytes.last(), Some(&0xff));
    assert_eq!(decode_sensml(&bytes)?, pack);
    assert!(matches!(decode(&bytes), Err(Error::Syntax { .. })));
    // SenSML readers also accept a definite array.
    let definite = encode(&pack)?;
    assert_eq!(decode_sensml(&definite)?, pack);
    let mut out = Vec::new();
    let mut s = StreamEncoder::new();
    for r in &pack {
        s.record(r, &mut out)?;
    }
    s.finish(&mut out);
    assert_eq!(out, bytes);
    Ok(())
}

#[test]
fn stream_decoder_takes_bytes_as_they_come() -> Result<()> {
    let bytes = encode_sensml(&rfc_pack())?;
    let mut d = StreamDecoder::new();
    let mut got = Vec::new();
    for chunk in bytes.chunks(5) {
        d.push(chunk);
        while let Some(r) = d.next_record()? {
            got.push(r);
        }
    }
    assert!(d.is_finished());
    d.finish()?;
    assert_eq!(Pack::from(got), rfc_pack());
    Ok(())
}

#[test]
fn reader_and_fixed_buffer() -> Result<()> {
    let bytes = hex(RFC_SECTION_6);
    assert_eq!(RecordReader::new(&bytes).count(), 7);
    let mut buf = [0u8; 256];
    let n = encode_into(&rfc_pack(), &mut buf)?;
    assert_eq!(&buf[..n], &bytes[..]);
    let mut small = [0u8; 100];
    assert_eq!(encode_into(&rfc_pack(), &mut small), Err(Error::BufferFull));
    Ok(())
}
