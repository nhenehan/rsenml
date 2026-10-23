//! SenML JSON reading and writing, including the cases other implementations get wrong.

use rsenml_core::{Error, Format, Pack, Record, Result, Value};
use rsenml_json::{RecordReader, StreamDecoder, StreamEncoder, decode, decode_unchecked, encode};

fn syntax(input: &str) -> bool {
    matches!(
        decode_unchecked(input.as_bytes()),
        Err(Error::Syntax { .. })
    )
}

#[test]
fn rfc8428_5_1_2_round_trips_byte_for_byte() -> Result<()> {
    let text = r#"[{"bn":"urn:dev:ow:10e2073a01080063:","n":"voltage","u":"V","v":120.1},{"n":"current","u":"A","v":1.2}]"#;
    let pack = decode(text.as_bytes())?;
    assert_eq!(pack.len(), 2);
    assert_eq!(encode(&pack)?, text.as_bytes());
    Ok(())
}

#[test]
fn rfc8428_5_1_5_data_types() -> Result<()> {
    let pack = decode(
        br#"[{"bn":"urn:dev:ow:10e2073a01080063:","n":"temp","u":"Cel","v":23.1},
             {"n":"label","vs":"Machine Room"},
             {"n":"open","vb":false},
             {"n":"nfc-reader","vd":"aGkgCg"}]"#,
    )?;
    let values: Vec<_> = pack.iter().map(|r| r.value.clone()).collect();
    assert_eq!(
        values,
        [
            Some(Value::Number(23.1)),
            Some(Value::String("Machine Room".into())),
            Some(Value::Bool(false)),
            Some(Value::Data(b"hi \n".to_vec().into())),
        ]
    );
    Ok(())
}

#[test]
fn numbers_keep_their_value_in_shortest_form() -> Result<()> {
    let pack = decode(br#"[{"n":"a","bt":1.276020076001e+09,"v":20.0,"t":-5}]"#)?;
    let r = &pack.records[0];
    assert_eq!(r.base_time, Some(1_276_020_076.001));
    assert_eq!(
        encode(&pack)?,
        br#"[{"bt":1276020076.001,"n":"a","t":-5,"v":20}]"#
    );
    Ok(())
}

#[test]
fn escapes_in_strings_and_keys() -> Result<()> {
    let input = "[{\"\\u006e\":\"a\\/b\",\"vs\":\"\\\"q\\\"\\n\\u00e9\\ud83d\\ude00\"}]";
    let pack = decode(input.as_bytes())?;
    let r = &pack.records[0];
    assert_eq!(r.name.as_deref(), Some("a/b"));
    assert_eq!(
        r.value,
        Some(Value::String("\"q\"\n\u{e9}\u{1f600}".into()))
    );
    let again = decode(&encode(&pack)?)?;
    assert_eq!(again, pack);
    Ok(())
}

#[test]
fn json_grammar_is_enforced() {
    for bad in [
        r#"[{"n":"a","v":+1}]"#,
        r#"[{"n":"a","v":.5}]"#,
        r#"[{"n":"a","v":01}]"#,
        r#"[{"n":"a","v":1.}]"#,
        r#"[{"n":"a","v":1e}]"#,
        r#"[{"n":"a","v":NaN}]"#,
        r#"[{"n":"a","v":1},]"#,
        r#"[{"n":"a","v":1,}]"#,
        r#"[{"n":"a" "v":1}]"#,
        r#"[{"n":"a","vs":"\x"}]"#,
        r#"[{"n":"a","vs":"\ud800"}]"#,
        r#"{"n":"a","v":1}"#,
        r#"[{"n":"a","v":1}] x"#,
        r#"[{"n":"a","v":1}][]"#,
        "[{\"n\":\"a\",\"vs\":\"tab\there\"}]",
    ] {
        assert!(syntax(bad), "{bad}");
    }
}

#[test]
fn truncated_input_is_incomplete() {
    for cut in [
        r#"["#,
        r#"[{"n":"a","v":1"#,
        r#"[{"n":"a","v":1},"#,
        r#"[{"n":"a","v":1}"#,
    ] {
        assert_eq!(
            decode_unchecked(cut.as_bytes()),
            Err(Error::Incomplete {
                format: Format::Json
            }),
            "{cut}"
        );
    }
}

#[test]
fn record_rules() {
    assert_eq!(
        decode(br#"[{"n":"a","v":1,"crit_":true}]"#),
        Err(Error::MustUnderstand {
            record: 0,
            label: "crit_".into()
        })
    );
    // Must-understand labels are refused even when their value is skipped.
    assert!(matches!(
        decode(br#"[{"n":"a","v":1,"x_":[1,{"y":2}]}]"#),
        Err(Error::MustUnderstand { .. })
    ));
    assert_eq!(
        decode(br#"[{"n":"a","v":1,"vs":"x"}]"#),
        Err(Error::MultipleValues { record: 0 })
    );
    assert!(matches!(
        decode(br#"[{"n":"a","n":"b","v":1}]"#),
        Err(Error::DuplicateLabel { record: 0, .. })
    ));
    assert_eq!(
        decode(br#"[{"n":"a","v":"1"}]"#),
        Err(Error::WrongType {
            record: 0,
            label: "v"
        })
    );
    assert_eq!(
        decode(br#"[{"n":"a","vd":"aGkgCg=="}]"#),
        Err(Error::InvalidBase64 { record: 0 })
    );
    assert_eq!(
        decode(br#"[{"n":"a","v":1e400}]"#),
        Err(Error::NonFinite {
            record: 0,
            label: "v".into()
        })
    );
    assert_eq!(
        decode(br#"[{"n":"a","v":1,"bver":11}]"#),
        Err(Error::UnsupportedVersion {
            record: 0,
            version: 11
        })
    );
    assert_eq!(decode(b"[]"), Err(Error::EmptyPack));
}

#[test]
fn unknown_labels_are_kept_or_skipped() -> Result<()> {
    let pack = decode(
        br#"[{"n":"a","v":1,"foo":"x","bar":2.5,"baz":true,"nested":{"a":[1,2]},"z":null}]"#,
    )?;
    let labels: Vec<_> = pack.records[0]
        .extensions
        .iter()
        .map(|e| e.label.as_ref())
        .collect();
    assert_eq!(labels, ["foo", "bar", "baz"]);
    assert_eq!(
        encode(&pack)?,
        br#"[{"n":"a","v":1,"foo":"x","bar":2.5,"baz":true}]"#
    );
    Ok(())
}

#[test]
fn reader_yields_records_one_by_one() -> Result<()> {
    let input = br#" [ {"n":"a","v":1} , {"n":"b","v":2} ] "#;
    let names: Vec<_> = RecordReader::new(input)
        .map(|r| r.map(|r| r.name.map(|n| n.into_owned())))
        .collect::<Result<_>>()?;
    assert_eq!(names, [Some("a".to_string()), Some("b".to_string())]);
    Ok(())
}

#[test]
fn stream_decoder_takes_bytes_as_they_come() -> Result<()> {
    let text = br#"[{"bn":"urn:dev:ow:10e2073a01080063","bt":1.320067464e+09,"bu":"%RH","v":21.2},{"t":10,"v":21.3},{"t":20,"vs":"a}]\"b"}]"#;
    let mut d = StreamDecoder::new();
    let mut got = Vec::new();
    for chunk in text.chunks(3) {
        d.push(chunk);
        while let Some(r) = d.next_record()? {
            got.push(r);
        }
    }
    assert!(d.is_finished());
    d.finish()?;
    assert_eq!(got.len(), 3);
    assert_eq!(got[2].value, Some(Value::String("a}]\"b".into())));
    Ok(())
}

#[test]
fn stream_encoder_matches_pack_encoding() -> Result<()> {
    let pack: Pack = [
        Record::new()
            .with_base_name("dev:")
            .with_name("a")
            .with_value(1.0),
        Record::new().with_name("b").with_value(2.0),
    ]
    .into_iter()
    .collect();
    let mut out = Vec::new();
    let mut s = StreamEncoder::new();
    for r in &pack {
        s.record(r, &mut out)?;
    }
    s.finish(&mut out);
    assert_eq!(out, encode(&pack)?);
    Ok(())
}

#[test]
fn encode_into_a_fixed_buffer() -> Result<()> {
    let pack = decode(br#"[{"n":"a","v":1}]"#)?;
    let mut buf = [0u8; 32];
    let n = rsenml_json::encode_into(&pack, &mut buf)?;
    assert_eq!(&buf[..n], br#"[{"n":"a","v":1}]"#);
    let mut small = [0u8; 8];
    assert_eq!(
        rsenml_json::encode_into(&pack, &mut small),
        Err(Error::BufferFull)
    );
    Ok(())
}
