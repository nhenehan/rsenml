//! SenML EXI reading and writing.

use rexi_core::{Encoder, Event, ExiOptions, HeaderConfig, QName, SchemaId, Value as ExiValue};
use rsenml_core::{Error, Format, Pack, Record, Result, Value};
use rsenml_exi::{
    Alignment, DecodeOptions, EncodeOptions, Extensions, RecordReader, StreamEncoder, decode,
    decode_with, encode, encode_into, encode_with,
};

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .filter_map(|b| u8::from_str_radix(b, 16).ok())
        .collect()
}

/// RFC 8428 section 8, bit-packed (0x3f bytes).
const RFC_BIT_PACKED: &str = "\
a0 30 0d 84 80 f3 ab 93 71 d3 23 2b b1 d3 7b b9
d1 89 83 29 91 81 b9 9b 09 81 89 81 c1 81 81 b1
99 d2 84 bb 37 b6 3a 30 b3 b2 90 1a b1 58 84 c0
33 04 b1 ba b9 39 32 b7 3a 10 1a 09 06 40 38";

/// RFC 8428 section 8, byte-aligned (0x31 bytes).
const RFC_BYTE_ALIGNED: &str = "\
a0 00 48 80 6c 20 01 06 1d 75 72 6e 3a 64 65 76
3a 6f 77 3a 31 30 65 32 30 37 33 61 30 31 30 38
30 30 36 33 02 05 43 65 6c 01 00 e7 01 01 00 03
01";

fn bit_packed_pack() -> Pack<'static> {
    Pack::from(vec![
        Record::new()
            .with_base_name("urn:dev:ow:10e2073a01080063:")
            .with_name("voltage")
            .with_unit("V")
            .with_value(120.1),
        Record::new()
            .with_name("current")
            .with_unit("A")
            .with_value(1.2),
    ])
}

fn byte_aligned_pack() -> Pack<'static> {
    Pack::from(vec![
        Record::new()
            .with_name("urn:dev:ow:10e2073a01080063")
            .with_unit("Cel")
            .with_value(23.1),
    ])
}

const BYTE_ALIGNED: EncodeOptions = EncodeOptions::new().with_alignment(Alignment::ByteAligned);

#[test]
fn rfc8428_section_8_bit_packed_both_ways() -> Result<()> {
    let bytes = hex(RFC_BIT_PACKED);
    assert_eq!(bytes.len(), 0x3f);
    assert_eq!(decode(&bytes)?, bit_packed_pack());
    assert_eq!(encode(&bit_packed_pack())?, bytes);
    Ok(())
}

#[test]
fn rfc8428_section_8_byte_aligned_both_ways() -> Result<()> {
    let bytes = hex(RFC_BYTE_ALIGNED);
    assert_eq!(bytes.len(), 0x31);
    assert_eq!(decode(&bytes)?, byte_aligned_pack());
    assert_eq!(encode_with(&byte_aligned_pack(), &BYTE_ALIGNED)?, bytes);
    Ok(())
}

#[test]
fn every_field_and_value_type_round_trips() -> Result<()> {
    let pack = Pack::from(vec![
        Record::new()
            .with_base_name("urn:dev:ow:10e2073a01080063:")
            .with_base_time(1_276_020_076.001)
            .with_base_unit("A")
            .with_base_value(0.5)
            .with_base_sum(-2.0)
            .with_base_version(5)
            .with_name("temp")
            .with_unit("Cel")
            .with_value(23.1)
            .with_sum(1e300)
            .with_time(-5.0)
            .with_update_time(60.0),
        Record::new()
            .with_name("label")
            .with_string_value("Machine Room"),
        Record::new().with_name("open").with_bool_value(false),
        Record::new()
            .with_name("nfc-reader")
            .with_data_value(b"hi \n".to_vec()),
        Record::new().with_name("tiny").with_value(5e-324),
    ]);
    for options in [EncodeOptions::default(), BYTE_ALIGNED] {
        let bytes = encode_with(&pack, &options)?;
        assert_eq!(decode(&bytes)?, pack, "{options:?}");
    }
    Ok(())
}

#[test]
fn extensions_are_dropped_or_refused() -> Result<()> {
    let pack = Pack::from(vec![
        Record::new()
            .with_name("a")
            .with_value(1.0)
            .with_extension("foo", Value::Bool(true)),
    ]);
    let bytes = encode(&pack)?;
    assert!(decode(&bytes)?.records[0].extensions.is_empty());
    let refuse = EncodeOptions::new().with_extensions(Extensions::Error);
    assert!(matches!(
        encode_with(&pack, &refuse),
        Err(Error::Unrepresentable { .. })
    ));
    Ok(())
}

#[test]
fn cookie_and_fixed_buffer() -> Result<()> {
    let with_cookie = EncodeOptions::new().with_cookie(true);
    let bytes = encode_with(&bit_packed_pack(), &with_cookie)?;
    assert!(bytes.starts_with(b"$EXI"));
    assert_eq!(decode(&bytes)?, bit_packed_pack());
    let mut buf = [0u8; 128];
    let n = encode_into(&bit_packed_pack(), &mut buf, &EncodeOptions::default())?;
    assert_eq!(&buf[..n], &hex(RFC_BIT_PACKED)[..]);
    let mut small = [0u8; 16];
    assert_eq!(
        encode_into(&bit_packed_pack(), &mut small, &EncodeOptions::default()),
        Err(Error::BufferFull)
    );
    Ok(())
}

/// A stream of the SenML schema written by rexi directly, with chosen options.
fn raw_stream(options: ExiOptions, include_options: bool) -> Vec<u8> {
    let header = HeaderConfig::new().with_include_options(include_options);
    let ns = rsenml_core::XML_NAMESPACE;
    let run = || -> rexi_core::ExiResult<Vec<u8>> {
        let mut e = Encoder::new(Vec::new(), &rsenml_exi::tables::GRAMMARS, options, header)?;
        for ev in [
            Event::StartDocument,
            Event::StartElement(QName::new(ns, "sensml")),
            Event::StartElement(QName::new(ns, "senml")),
            Event::Attribute(QName::new("", "n"), ExiValue::String("a")),
            Event::Attribute(QName::new("", "v"), ExiValue::Lexical("1")),
            Event::EndElement,
            Event::EndElement,
            Event::EndDocument,
        ] {
            e.push(&ev)?;
        }
        e.finish()
    };
    run().unwrap_or_default()
}

#[test]
fn schema_id_is_checked() -> Result<()> {
    let strict = |id: SchemaId| {
        let mut o = ExiOptions::strict();
        o.schema_id = id;
        o
    };
    let other = raw_stream(strict(SchemaId::Id("b".into())), true);
    assert_eq!(
        decode(&other),
        Err(Error::SchemaId {
            found: Some("b".into())
        })
    );
    let absent = raw_stream(strict(SchemaId::Absent), true);
    assert_eq!(decode(&absent), Err(Error::SchemaId { found: None }));
    // Options out of band: refused unless the caller supplies them.
    let bare = raw_stream(strict(SchemaId::Id("a".into())), false);
    assert_eq!(decode(&bare), Err(Error::SchemaId { found: None }));
    let oob = DecodeOptions::new().with_out_of_band(Some(Alignment::BitPacked));
    assert_eq!(decode_with(&bare, &oob)?.len(), 1);
    Ok(())
}

#[test]
fn malformed_streams() {
    assert!(matches!(decode(&[0x00, 0x01]), Err(Error::Syntax { .. })));
    let bytes = hex(RFC_BIT_PACKED);
    assert_eq!(
        decode(&bytes[..bytes.len() / 2]),
        Err(Error::Incomplete {
            format: Format::Exi
        })
    );
    assert_eq!(
        decode(&[]),
        Err(Error::Incomplete {
            format: Format::Exi
        })
    );
}

#[test]
fn writer_and_reader_work_record_by_record() -> Result<()> {
    let mut w = StreamEncoder::new(Vec::new(), &EncodeOptions::default())?;
    for r in &bit_packed_pack() {
        w.record(r)?;
    }
    let bytes = w.finish()?;
    assert_eq!(bytes, hex(RFC_BIT_PACKED));
    let names: Vec<_> = RecordReader::new(&bytes, &DecodeOptions::default())?
        .map(|r| r.map(|r| r.name))
        .collect::<Result<_>>()?;
    assert_eq!(names, [Some("voltage".into()), Some("current".into())]);
    Ok(())
}

#[test]
fn bver_must_fit_xs_int() {
    let pack = Pack::from(vec![
        Record::new()
            .with_name("a")
            .with_value(1.0)
            .with_base_version(10),
    ]);
    assert!(encode(&pack).is_ok());
    let mut big = pack.clone();
    big.records[0].base_version = Some(1 << 40);
    // Validation refuses it first (versions above 10).
    assert!(matches!(
        encode(&big),
        Err(Error::UnsupportedVersion { .. })
    ));
}

#[test]
fn unchecked_encode_skips_validation_only() -> Result<()> {
    let pack = bit_packed_pack();
    assert_eq!(rsenml_exi::encode_unchecked(&pack)?, encode(&pack)?);
    // A record with neither value nor sum: invalid SenML, but EXI can carry it.
    let invalid = Pack::from(vec![Record::new().with_name("a").with_time(1.0)]);
    assert_eq!(encode(&invalid), Err(Error::MissingValue { record: 0 }));
    let bytes = rsenml_exi::encode_unchecked(&invalid)?;
    assert_eq!(rsenml_exi::decode_unchecked(&bytes)?, invalid);
    // Values EXI cannot carry are still refused.
    let nan = Pack::from(vec![Record::new().with_name("a").with_value(f64::NAN)]);
    assert!(matches!(
        rsenml_exi::encode_unchecked(&nan),
        Err(Error::NonFinite { .. })
    ));
    Ok(())
}
