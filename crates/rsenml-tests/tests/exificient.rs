//! rsenml-exi against EXIficient 1.0.7: for every case, the bytes rsenml-exi
//! writes equal the bytes EXIficient writes for the same pack as SenML XML, and
//! rsenml-exi reads EXIficient's bytes back to the pack. The reference bytes are
//! in fixtures/exificient (scripts/gen-exi-fixtures.sh).

use std::path::PathBuf;

use rsenml::Result;
use rsenml::exi::{Alignment, EncodeOptions};
use rsenml_tests::{all_cases, exi_view, root, same_pack};

fn fixtures() -> PathBuf {
    root().join("crates/rsenml-tests/fixtures/exificient")
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(fixtures().join(name)).unwrap_or_default()
}

#[test]
fn every_case_matches_exificient_byte_for_byte() -> Result<()> {
    let manifest = std::fs::read_to_string(fixtures().join("manifest.tsv")).unwrap_or_default();
    let mut checked = 0;
    for (name, pack) in all_cases() {
        // The fixture's input is what rsenml-xml writes today.
        assert_eq!(
            read(&format!("{name}.xml")),
            rsenml::xml::encode(&pack)?,
            "{name}: stale fixture"
        );
        for (profile, alignment) in [("bp", Alignment::BitPacked), ("ba", Alignment::ByteAligned)] {
            let reference = read(&format!("{name}.{profile}.exi"));
            assert!(!reference.is_empty(), "{name}.{profile}: missing fixture");
            assert!(manifest.contains(&format!("{name}\t{profile}\t{}\t", reference.len())));
            let options = EncodeOptions::new().with_alignment(alignment);
            assert_eq!(
                rsenml::exi::encode_with(&pack, &options)?,
                reference,
                "{name}.{profile}"
            );
            assert!(
                same_pack(&rsenml::exi::decode(&reference)?, &exi_view(&pack)),
                "{name}.{profile}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 32);
    Ok(())
}
