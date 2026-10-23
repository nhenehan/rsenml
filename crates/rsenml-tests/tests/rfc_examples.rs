//! Every example of RFC 8428, read from the RFC text, in every representation.

use rsenml::{Error, Pack, Result, Value};
use rsenml_tests::{hex_dumps, json_examples, xml_examples};

fn example(name: &str) -> Result<Pack<'static>> {
    let ex = json_examples()
        .into_iter()
        .find(|e| e.name == name)
        .ok_or(Error::EmptyPack)?;
    rsenml::json::decode(ex.json.as_bytes())
}

#[test]
fn every_json_example_is_valid() -> Result<()> {
    for ex in json_examples() {
        let pack = rsenml::json::decode(ex.json.as_bytes())?;
        assert!(!pack.is_empty(), "{}", ex.name);
    }
    Ok(())
}

#[test]
fn the_sensml_example_streams_record_by_record() -> Result<()> {
    // Section 5.1.2's stream "may be reported at the time it was measured":
    // feed it without the closing bracket and every record still comes out.
    let ex = json_examples()
        .into_iter()
        .find(|e| e.partial_stream)
        .ok_or(Error::EmptyPack)?;
    let open = ex.json.trim_end().trim_end_matches(']');
    let mut d = rsenml::json::StreamDecoder::new();
    d.push(open.as_bytes());
    let mut n = 0;
    while d.next_record()?.is_some() {
        n += 1;
    }
    assert_eq!(n, 9);
    assert!(!d.is_finished());
    Ok(())
}

#[test]
fn section_5_1_4_is_section_5_1_3_resolved() -> Result<()> {
    let resolved = example("5.1.3")?.resolve(None)?;
    let expected = example("5.1.4")?;
    assert_eq!(resolved.len(), expected.len());
    for (r, e) in resolved.iter().zip(&expected) {
        assert_eq!(Some(r.name.as_str()), e.name.as_deref());
        assert_eq!(r.unit.as_deref(), e.unit.as_deref());
        assert_eq!(Some(r.time), e.time);
        assert_eq!(r.value, e.value);
    }
    // And the resolved records written back are exactly section 5.1.4.
    let back: Pack = resolved.into_iter().map(|r| r.into_record()).collect();
    assert_eq!(back, expected);
    Ok(())
}

#[test]
fn section_5_1_6_mixes_base_names() -> Result<()> {
    let resolved = example("5.1.6")?.resolve(None)?;
    let names: Vec<_> = resolved.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "2001:db8::2/temperature",
            "2001:db8::2/humidity",
            "2001:db8::1/temperature",
            "2001:db8::1/humidity"
        ]
    );
    // "the last record would use the Base Name from the 3rd record but the Base
    // Time from the first record"
    assert!(resolved.iter().all(|r| r.time == 1.320078429e9));
    Ok(())
}

#[test]
fn section_5_1_7_actuation_times() -> Result<()> {
    let now = 1_320_078_429.0;
    let resolved = example("5.1.7c")?.resolve(Some(now))?;
    let times: Vec<_> = resolved.iter().map(|r| r.time).collect();
    assert_eq!(times, [now, now, now + 0.1, now + 0.1]);
    // 5.1.7a starts with a record of base fields only.
    assert_eq!(example("5.1.7a")?.resolve(Some(now))?.len(), 3);
    Ok(())
}

#[test]
fn section_6_cbor_is_section_5_1_2_with_an_explicit_zero_time() -> Result<()> {
    let dumps = hex_dumps("6");
    let cbor = rsenml::cbor::decode(&dumps[0])?;
    let mut json = example("5.1.2b")?;
    // The CBOR example writes `6: 0` on the last record; JSON omits the time.
    if let Some(last) = json.records.last_mut() {
        last.time = Some(0.0);
    }
    assert_eq!(cbor, json);
    assert_eq!(rsenml::cbor::encode(&json)?, dumps[0]);
    let now = Some(1_000_000_000.0);
    assert_eq!(cbor.resolve(now)?, example("5.1.2b")?.resolve(now)?);
    Ok(())
}

#[test]
fn section_7_xml_is_section_5_1_2() -> Result<()> {
    let xml = xml_examples("7");
    assert_eq!(rsenml::xml::decode(xml[0].as_bytes())?, example("5.1.2b")?);
    Ok(())
}

#[test]
fn section_8_exi_dumps_are_its_xml_examples() -> Result<()> {
    let xml = xml_examples("8");
    let dumps = hex_dumps("8");
    let bit_packed = rsenml::xml::decode(xml[0].as_bytes())?;
    let byte_aligned = rsenml::xml::decode(xml[1].as_bytes())?;
    // "the same information as the first example in Section 5.1.2"
    assert_eq!(bit_packed, example("5.1.2a")?);
    assert_eq!(rsenml::exi::decode(&dumps[0])?, bit_packed);
    assert_eq!(rsenml::exi::decode(&dumps[1])?, byte_aligned);
    assert_eq!(rsenml::exi::encode(&bit_packed)?, dumps[0]);
    let ba = rsenml::exi::EncodeOptions::new().with_alignment(rsenml::exi::Alignment::ByteAligned);
    assert_eq!(rsenml::exi::encode_with(&byte_aligned, &ba)?, dumps[1]);
    Ok(())
}

#[test]
fn section_8_hand_coded_sensor_value() -> Result<()> {
    // "replacing the value 0xe7 0x01 at location 0x2b and 0x2c with the current
    // temperature" in tenths of a degree: 23.1 -> 231 -> e7 01; try 55.0 -> 550.
    let mut bytes = hex_dumps("8")[1].clone();
    let tenths: u32 = 550;
    bytes[0x2b] = (tenths & 0x7f) as u8 | 0x80;
    bytes[0x2c] = (tenths >> 7) as u8;
    let pack = rsenml::exi::decode(&bytes)?;
    assert_eq!(pack.records[0].value, Some(Value::Number(55.0)));
    Ok(())
}

/// Table 3 lists the sizes of the section 5.1.3 example; rsenml writes JSON and
/// XML without the RFC's whitespace, so those come out smaller.
#[test]
fn table_3_sizes() -> Result<()> {
    let pack = example("5.1.3")?;
    let json = rsenml::json::encode(&pack)?.len();
    let xml = rsenml::xml::encode(&pack)?.len();
    let cbor = rsenml::cbor::encode(&pack)?.len();
    let exi = rsenml::exi::encode(&pack)?.len();
    println!(
        "5.1.3: JSON {json} (RFC 573), XML {xml} (RFC 649), CBOR {cbor} (RFC 254), EXI {exi} (RFC 161)"
    );
    assert!(json < 573 && xml < 649 && cbor <= 254 && exi <= 161);
    Ok(())
}
