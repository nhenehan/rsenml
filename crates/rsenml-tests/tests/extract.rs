//! The excerpts in `fixtures/rfc8428` are exactly what RFC 8428 says.
//!
//! This needs the full RFC from `scripts/fetch-standards.sh`. Without it the test
//! is skipped, unless `RSENML_REQUIRE_STANDARDS` is set (the full gate and CI's
//! `exificient` job set it). `RSENML_WRITE_EXCERPTS=1` writes the excerpts again
//! instead of comparing them.

use rsenml_tests::{excerpts_dir, rfc_text};

#[test]
fn excerpts_are_the_rfc_text() -> Result<(), Box<dyn std::error::Error>> {
    let Some(text) = rfc_text::rfc_text() else {
        assert!(
            std::env::var_os("RSENML_REQUIRE_STANDARDS").is_none(),
            "RFC 8428 is not in {}: run scripts/fetch-standards.sh",
            rfc_text::standards_dir().display()
        );
        eprintln!("RFC 8428 not fetched; skipping the excerpt check");
        return Ok(());
    };
    let excerpts = rfc_text::excerpts(&text);
    let names: Vec<_> = excerpts.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "5.1.1.json",
            "5.1.2a.json",
            "5.1.2b.json",
            "5.1.2c.txt",
            "5.1.3.json",
            "5.1.4.json",
            "5.1.5.json",
            "5.1.6.json",
            "5.1.7a.json",
            "5.1.7b.json",
            "5.1.7c.json",
            "6.hex",
            "7.xml",
            "8a.xml",
            "8b.xml",
            "8a.hex",
            "8b.hex",
            "senml.xsd"
        ]
    );
    let write = std::env::var_os("RSENML_WRITE_EXCERPTS").is_some();
    for (name, want) in &excerpts {
        let path = excerpts_dir().join(name);
        if write {
            std::fs::write(&path, want)?;
        } else {
            let have = std::fs::read_to_string(&path).unwrap_or_default();
            assert_eq!(&have, want, "{name} differs from RFC 8428");
        }
    }
    // Every excerpt file is one the RFC text gives (none left over).
    let mut files = Vec::new();
    for entry in std::fs::read_dir(excerpts_dir())? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name != "README.md" {
            files.push(name);
        }
    }
    files.sort();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(files, sorted);
    // The schema fetch-standards.sh cuts out for itself is the committed one.
    let fetched = std::fs::read_to_string(rfc_text::standards_dir().join("senml.xsd"));
    if let Ok(fetched) = fetched {
        assert_eq!(fetched, rsenml_tests::rfc_xsd());
    }
    Ok(())
}

#[test]
fn every_rfc_vector_is_read() {
    let names: Vec<_> = rsenml_tests::json_examples()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, rsenml_tests::JSON_EXAMPLES);
    assert_eq!(rsenml_tests::xml_examples("7").len(), 1);
    assert_eq!(rsenml_tests::xml_examples("8").len(), 2);
    let cbor = rsenml_tests::hex_dumps("6");
    assert_eq!(cbor.iter().map(Vec::len).collect::<Vec<_>>(), [0xc3]);
    let exi = rsenml_tests::hex_dumps("8");
    assert_eq!(exi.iter().map(Vec::len).collect::<Vec<_>>(), [0x3f, 0x31]);
}
