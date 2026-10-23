//! The RFC vectors are found where expected.

#[test]
fn extraction_finds_every_rfc_vector() {
    let names: Vec<_> = rsenml_tests::json_examples()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(
        names,
        [
            "5.1.1", "5.1.2a", "5.1.2b", "5.1.2c", "5.1.3", "5.1.4", "5.1.5", "5.1.6", "5.1.7a",
            "5.1.7b", "5.1.7c"
        ]
    );
    assert_eq!(rsenml_tests::xml_examples("7").len(), 1);
    assert_eq!(rsenml_tests::xml_examples("8").len(), 2);
    let cbor = rsenml_tests::hex_dumps("6");
    assert_eq!(cbor.iter().map(Vec::len).collect::<Vec<_>>(), [0xc3]);
    let exi = rsenml_tests::hex_dumps("8");
    assert_eq!(exi.iter().map(Vec::len).collect::<Vec<_>>(), [0x3f, 0x31]);
    let xsd =
        std::fs::read_to_string(rsenml_tests::root().join("spec/senml.xsd")).unwrap_or_default();
    assert_eq!(rsenml_tests::rfc_xsd(), xsd);
}
