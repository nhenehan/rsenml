//! Every case through every representation and back, plus conversions between
//! representations.

use rsenml::{MediaType, Pack, Result};
use rsenml_tests::{all_cases, exi_view, same_pack};

fn bit_packed(p: &Pack<'_>) -> Result<Vec<u8>> {
    rsenml::exi::encode(p)
}

fn byte_aligned(p: &Pack<'_>) -> Result<Vec<u8>> {
    rsenml::exi::encode_with(
        p,
        &rsenml::exi::EncodeOptions::new().with_alignment(rsenml::exi::Alignment::ByteAligned),
    )
}

#[test]
fn every_case_round_trips_in_every_media_type() -> Result<()> {
    for (name, pack) in all_cases() {
        for media in MediaType::ALL {
            let bytes = rsenml::encode(media, &pack)?;
            let back = rsenml::decode(media, &bytes)?;
            let expected = if media.format() == rsenml::Format::Exi {
                exi_view(&pack)
            } else {
                pack.clone()
            };
            assert!(same_pack(&back, &expected), "{name} via {media:?}");
        }
        for encode in [bit_packed, byte_aligned] {
            let bytes = encode(&pack)?;
            assert!(
                same_pack(&rsenml::exi::decode(&bytes)?, &exi_view(&pack)),
                "{name} via EXI"
            );
        }
    }
    Ok(())
}

#[test]
fn conversions_chain_without_loss() -> Result<()> {
    // JSON -> CBOR -> XML -> EXI -> JSON gives the same JSON bytes.
    for (name, pack) in all_cases() {
        let json = rsenml::json::encode(&exi_view(&pack))?;
        let a = rsenml::json::decode(&json)?;
        let cbor = rsenml::cbor::encode(&a)?;
        let b = rsenml::cbor::decode(&cbor)?;
        let xml = rsenml::xml::encode(&b)?;
        let c = rsenml::xml::decode(&xml)?;
        let exi = rsenml::exi::encode(&c)?;
        let d = rsenml::exi::decode(&exi)?;
        assert_eq!(rsenml::json::encode(&d)?, json, "{name}");
    }
    Ok(())
}

#[test]
fn negative_zero_survives_every_representation_but_exi() -> Result<()> {
    let pack = Pack::from(vec![rsenml::Record::new().with_name("a").with_value(-0.0)]);
    for media in [
        MediaType::SenmlJson,
        MediaType::SenmlCbor,
        MediaType::SenmlXml,
    ] {
        let bytes = rsenml::encode(media, &pack)?;
        let back = rsenml::decode(media, &bytes)?;
        assert!(same_pack(&back, &pack), "{media:?}");
    }
    let back = rsenml::exi::decode(&rsenml::exi::encode(&pack)?)?;
    assert_eq!(back.records[0].value, Some(rsenml::Value::Number(0.0)));
    assert!(!same_pack(&back, &pack));
    Ok(())
}

#[test]
fn extensions_survive_json_cbor_and_xml() -> Result<()> {
    let json = br#"[{"n":"a","v":1,"origin":"lab-3","weight":0.5,"calibrated":true}]"#;
    let pack = rsenml::json::decode(json)?;
    let cbor = rsenml::cbor::encode(&pack)?;
    let via_cbor = rsenml::cbor::decode(&cbor)?.into_owned();
    assert_eq!(via_cbor, pack);
    assert_eq!(rsenml::json::encode(&via_cbor)?, json);
    // XML has only strings, so typed extensions come back as text.
    let xml = rsenml::xml::encode(&pack)?;
    let via_xml = rsenml::xml::decode(&xml)?.into_owned();
    let texts: Vec<_> = via_xml.records[0]
        .extensions
        .iter()
        .map(|e| (e.label.to_string(), e.value.clone()))
        .collect();
    assert_eq!(
        texts,
        [
            ("origin".to_string(), rsenml::Value::String("lab-3".into())),
            ("weight".to_string(), rsenml::Value::String("0.5".into())),
            (
                "calibrated".to_string(),
                rsenml::Value::String("true".into())
            ),
        ]
    );
    Ok(())
}

#[test]
fn sizes_per_representation() -> Result<()> {
    println!(
        "{:<24} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "case", "JSON", "XML", "CBOR", "EXI", "EXI-ba"
    );
    for (name, pack) in all_cases() {
        let sizes = [
            rsenml::json::encode(&pack)?.len(),
            rsenml::xml::encode(&pack)?.len(),
            rsenml::cbor::encode(&pack)?.len(),
            bit_packed(&pack)?.len(),
            byte_aligned(&pack)?.len(),
        ];
        println!(
            "{name:<24} {:>6} {:>6} {:>6} {:>6} {:>6}",
            sizes[0], sizes[1], sizes[2], sizes[3], sizes[4]
        );
        // The binary forms are never larger than the text forms.
        assert!(
            sizes[2] <= sizes[0] && sizes[3] <= sizes[2].max(sizes[4]),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn unchecked_encoding_equals_checked_encoding_for_valid_packs() -> Result<()> {
    for (name, pack) in all_cases() {
        for media in MediaType::ALL {
            assert_eq!(
                rsenml::encode_unchecked(media, &pack)?,
                rsenml::encode(media, &pack)?,
                "{name} {media:?}"
            );
        }
    }
    Ok(())
}
