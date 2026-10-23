//! SenML XML reading and writing.

use rsenml_core::{Error, Format, Pack, Record, Result, Value};
use rsenml_xml::{RecordReader, decode, decode_unchecked, encode};

/// RFC 8428 section 7.
const RFC_SECTION_7: &str = r#"<sensml xmlns="urn:ietf:params:xml:ns:senml">
  <senml bn="urn:dev:ow:10e2073a0108006:" bt="1.276020076001e+09"
  bu="A" bver="5" n="voltage" u="V" v="120.1"></senml>
  <senml n="current" t="-5" v="1.2"></senml>
  <senml n="current" t="-4" v="1.3"></senml>
  <senml n="current" t="-3" v="1.4"></senml>
  <senml n="current" t="-2" v="1.5"></senml>
  <senml n="current" t="-1" v="1.6"></senml>
  <senml n="current" v="1.7"></senml>
</sensml>"#;

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
    ] {
        records.push(
            Record::new()
                .with_name("current")
                .with_time(t)
                .with_value(v),
        );
    }
    records.push(Record::new().with_name("current").with_value(1.7));
    Pack::from(records)
}

#[test]
fn rfc8428_section_7() -> Result<()> {
    let pack = decode(RFC_SECTION_7.as_bytes())?;
    assert_eq!(pack, rfc_pack());
    let bytes = encode(&pack)?;
    assert_eq!(decode(&bytes)?, pack);
    assert!(bytes.starts_with(
        br#"<sensml xmlns="urn:ietf:params:xml:ns:senml"><senml bn="urn:dev:ow:10e2073a0108006:" bt="1276020076.001" bu="A" bver="5" n="voltage" u="V" v="120.1"/>"#
    ));
    Ok(())
}

#[test]
fn namespaces_prefixes_and_foreign_attributes() -> Result<()> {
    let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- a comment --><?pi data?>
<s:sensml xmlns:s="urn:ietf:params:xml:ns:senml" xmlns:x="urn:other">
  <s:senml n="a" v="1" xml:lang="en" x:foo="ignored"/>
  <senml xmlns="urn:ietf:params:xml:ns:senml" n="b" vb="1"/>
</s:sensml>
<!-- trailing -->"#;
    let pack = decode(doc.as_bytes())?;
    assert_eq!(pack.len(), 2);
    assert!(pack.records[0].extensions.is_empty());
    assert_eq!(pack.records[1].value, Some(Value::Bool(true)));
    Ok(())
}

#[test]
fn references_and_attribute_normalization() -> Result<()> {
    let doc = "<sensml xmlns=\"urn:ietf:params:xml:ns:senml\"><senml n=\"a\" vs=\"&lt;&amp;&quot;&#xE9;&#10;tab\there\" v2=\"  x  \"/></sensml>";
    let pack = decode_unchecked(doc.as_bytes())?;
    let r = &pack.records[0];
    assert_eq!(r.value, Some(Value::String("<&\"\u{e9}\ntab here".into())));
    assert_eq!(r.extensions[0].value, Value::String("  x  ".into()));
    // Round trip keeps the newline through &#10;.
    let again = decode_unchecked(&encode(&pack)?)?.into_owned();
    assert_eq!(again, pack.clone().into_owned());
    Ok(())
}

#[test]
fn xsd_types() -> Result<()> {
    let one = |attrs: &str| {
        let doc = format!(
            r#"<sensml xmlns="urn:ietf:params:xml:ns:senml"><senml n="a" {attrs}/></sensml>"#
        );
        decode(doc.as_bytes()).map(Pack::into_owned)
    };
    assert_eq!(
        one(r#"v=" 1.5E2 ""#)?.records[0].value,
        Some(Value::Number(150.0))
    );
    assert_eq!(
        one(r#"v="+.5""#)?.records[0].value,
        Some(Value::Number(0.5))
    );
    assert_eq!(
        one(r#"v="-0""#)?.records[0].value,
        Some(Value::Number(-0.0))
    );
    assert_eq!(
        one(r#"vb="false""#)?.records[0].value,
        Some(Value::Bool(false))
    );
    assert_eq!(one(r#"v="1" bver="+7""#)?.records[0].base_version, Some(7));
    assert!(matches!(one(r#"v="INF""#), Err(Error::NonFinite { .. })));
    assert!(matches!(one(r#"v="NaN""#), Err(Error::NonFinite { .. })));
    assert!(matches!(one(r#"v="1e""#), Err(Error::WrongType { .. })));
    assert!(matches!(one(r#"v="0x10""#), Err(Error::WrongType { .. })));
    assert!(matches!(
        one(r#"v="1" bver="5.0""#),
        Err(Error::WrongType { .. })
    ));
    assert!(matches!(
        one(r#"v="1" bver="-1""#),
        Err(Error::InvalidVersion { .. })
    ));
    assert!(matches!(
        one(r#"v="1" bver="2147483648""#),
        Err(Error::WrongType { .. })
    ));
    assert!(matches!(one(r#"vb="yes""#), Err(Error::WrongType { .. })));
    assert!(matches!(
        one(r#"vd="aGkgCg==""#),
        Err(Error::InvalidBase64 { .. })
    ));
    assert!(matches!(
        one(r#"v="1" crit_="x""#),
        Err(Error::MustUnderstand { .. })
    ));
    assert!(matches!(
        one(r#"v="1" vs="x""#),
        Err(Error::MultipleValues { .. })
    ));
    Ok(())
}

#[test]
fn structure_is_enforced() {
    let ns = r#"xmlns="urn:ietf:params:xml:ns:senml""#;
    for bad in [
        r#"<sensml><senml n="a" v="1"/></sensml>"#.to_string(),
        r#"<sensml xmlns="urn:other"><senml n="a" v="1"/></sensml>"#.to_string(),
        format!(r#"<senml {ns} n="a" v="1"/>"#),
        format!(r#"<sensml {ns}><other n="a" v="1"/></sensml>"#),
        format!(r#"<sensml {ns}><senml n="a" v="1">text</senml></sensml>"#),
        format!(r#"<sensml {ns}><senml n="a" v="1"><senml/></senml></sensml>"#),
        format!(r#"<sensml {ns}>text<senml n="a" v="1"/></sensml>"#),
        format!(r#"<sensml {ns}><senml n="a" v="1"></senm></sensml>"#),
        format!(r#"<!DOCTYPE sensml [<!ENTITY x "y">]><sensml {ns}><senml n="a" v="1"/></sensml>"#),
        format!(
            r#"<?xml version="1.0" encoding="ISO-8859-1"?><sensml {ns}><senml n="a" v="1"/></sensml>"#
        ),
        format!(r#"<sensml {ns}><senml n="a" v="1"/></sensml><sensml {ns}/>"#),
        format!(r#"<sensml {ns}><senml n="a" vs="&bogus;"/></sensml>"#),
        format!(r#"<sensml {ns}><senml n="a" vs="&#0;"/></sensml>"#),
    ] {
        assert!(
            matches!(
                decode(bad.as_bytes()),
                Err(Error::Syntax { .. } | Error::Structure { .. })
            ),
            "{bad}: {:?}",
            decode(bad.as_bytes())
        );
    }
    assert!(matches!(decode(&[0xff, 0xfe]), Err(Error::Syntax { .. })));
    assert_eq!(
        decode(format!(r#"<sensml {ns}><senml n="a" v="1"/>"#).as_bytes()),
        Err(Error::Incomplete {
            format: Format::Xml
        })
    );
    assert_eq!(
        decode(format!(r#"<sensml {ns}/>"#).as_bytes()),
        Err(Error::EmptyPack)
    );
}

#[test]
fn writer_escapes_and_refuses() -> Result<()> {
    let pack = Pack::from(vec![
        Record::new()
            .with_name("a")
            .with_string_value("x\"<&\r\n\ty")
            .with_extension("ext-1", Value::Data(vec![0xfb, 0xff].into())),
    ]);
    let bytes = encode(&pack)?;
    assert!(
        String::from_utf8_lossy(&bytes)
            .contains(r#"vs="x&quot;&lt;&amp;&#13;&#10;&#9;y" ext-1="-_8""#)
    );
    let back = decode(&bytes)?;
    assert_eq!(back.records[0].value, pack.records[0].value);
    let bad = Pack::from(vec![
        Record::new().with_name("a").with_string_value("\u{1}"),
    ]);
    assert!(matches!(encode(&bad), Err(Error::Unrepresentable { .. })));
    let bad = Pack::from(vec![
        Record::new()
            .with_name("a")
            .with_value(1.0)
            .with_extension("1bad", Value::Bool(true)),
    ]);
    assert!(matches!(encode(&bad), Err(Error::Unrepresentable { .. })));
    // Any XML name the decoder reads must be written back, not only ASCII ones
    // (found by fuzzing: U+FEFF is a NameChar).
    let doc = "<sensml xmlns=\"urn:ietf:params:xml:ns:senml\"><senml n=\"a\" v=\"1\" u\u{feff}=\"V\" \u{e9}t\u{e9}=\"1\"/></sensml>";
    let pack = decode(doc.as_bytes())?;
    assert_eq!(decode(&encode(&pack)?)?, pack);
    Ok(())
}

#[test]
fn record_reader() -> Result<()> {
    let mut r = RecordReader::new(RFC_SECTION_7.as_bytes())?;
    let mut n = 0;
    while r.next_record()?.is_some() {
        n += 1;
    }
    r.finish()?;
    assert_eq!(n, 7);
    Ok(())
}

#[test]
fn encode_into_a_fixed_buffer() -> Result<()> {
    let pack = rfc_pack();
    let whole = encode(&pack)?;
    let mut buf = vec![0u8; whole.len()];
    let n = rsenml_xml::encode_into(&pack, &mut buf)?;
    assert_eq!(&buf[..n], &whole[..]);
    let mut small = vec![0u8; whole.len() - 1];
    assert_eq!(
        rsenml_xml::encode_into(&pack, &mut small),
        Err(Error::BufferFull)
    );
    Ok(())
}

#[test]
fn stream_encoder_matches_pack_encoding() -> Result<()> {
    let pack = rfc_pack();
    let mut out = Vec::new();
    let mut s = rsenml_xml::StreamEncoder::new();
    for r in &pack {
        s.record(r, &mut out)?;
    }
    s.finish(&mut out);
    assert_eq!(out, encode(&pack)?);
    let mut one = Vec::new();
    rsenml_xml::encode_record(&pack.records[1], 1, &mut one)?;
    assert_eq!(one, br#"<senml n="current" t="-5" v="1.2"/>"#);
    Ok(())
}
