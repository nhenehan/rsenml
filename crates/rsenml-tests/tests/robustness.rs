//! Decoders never panic: valid encodings of every case, truncated at every
//! length and randomly mutated, go through every decoder (each format's input
//! through every other format's decoder too). Errors are expected; a panic fails
//! the test and prints the input.
//!
//! `RSENML_MUTATIONS` sets the mutations per input (default 64).

use std::panic::{AssertUnwindSafe, catch_unwind};

use rsenml::Result;
use rsenml_tests::all_cases;

/// xorshift64*, deterministic.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn decode_everything(input: &[u8]) {
    let _ = rsenml::json::decode(input);
    let _ = rsenml::cbor::decode(input);
    let _ = rsenml::cbor::decode_sensml(input);
    let _ = rsenml::xml::decode(input);
    let _ = rsenml::exi::decode(input);
    let _ = rsenml::exi::decode_with(
        input,
        &rsenml::exi::DecodeOptions::new()
            .with_out_of_band(Some(rsenml::exi::Alignment::ByteAligned)),
    );
    let mut j = rsenml::json::StreamDecoder::new();
    let mut c = rsenml::cbor::StreamDecoder::new();
    for chunk in input.chunks(7) {
        j.push(chunk);
        c.push(chunk);
        while let Ok(Some(_)) = j.next_record() {}
        while let Ok(Some(_)) = c.next_record() {}
    }
    let _ = j.finish();
    let _ = c.finish();
}

fn check(input: &[u8], what: &str) {
    let ok = catch_unwind(AssertUnwindSafe(|| decode_everything(input))).is_ok();
    let hex: String = if ok {
        String::new()
    } else {
        input.iter().map(|b| format!("{b:02x}")).collect()
    };
    assert!(ok, "decoder panicked on {what}: {hex}");
}

fn mutate(rng: &mut Rng, input: &[u8]) -> Vec<u8> {
    let mut out = input.to_vec();
    for _ in 0..=rng.below(3) {
        match rng.below(4) {
            0 if !out.is_empty() => {
                let i = rng.below(out.len());
                out[i] ^= 1 << rng.below(8);
            }
            1 if !out.is_empty() => {
                let i = rng.below(out.len());
                out[i] = rng.next() as u8;
            }
            2 => {
                let i = rng.below(out.len() + 1);
                out.insert(i, rng.next() as u8);
            }
            _ if !out.is_empty() => {
                let i = rng.below(out.len());
                out.remove(i);
            }
            _ => {}
        }
    }
    out
}

#[test]
fn decoders_never_panic() -> Result<()> {
    let mutations: usize = std::env::var("RSENML_MUTATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(64);
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for (name, pack) in all_cases() {
        let ba =
            rsenml::exi::EncodeOptions::new().with_alignment(rsenml::exi::Alignment::ByteAligned);
        let inputs = [
            ("json", rsenml::json::encode(&pack)?),
            ("cbor", rsenml::cbor::encode(&pack)?),
            ("cbor-sensml", rsenml::cbor::encode_sensml(&pack)?),
            ("xml", rsenml::xml::encode(&pack)?),
            ("exi", rsenml::exi::encode(&pack)?),
            ("exi-ba", rsenml::exi::encode_with(&pack, &ba)?),
        ];
        for (format, bytes) in &inputs {
            // Every truncation of the small inputs, a sample of the large ones.
            let step = (bytes.len() / 200).max(1);
            for len in (0..bytes.len()).step_by(step) {
                check(&bytes[..len], &format!("{name}/{format} cut at {len}"));
            }
            for i in 0..mutations {
                let m = mutate(&mut rng, bytes);
                check(&m, &format!("{name}/{format} mutation {i}"));
            }
        }
    }
    Ok(())
}

#[test]
fn hostile_inputs() {
    let deep_json = format!(
        "[{{\"n\":\"a\",\"v\":1,\"x\":{}{}}}]",
        "[".repeat(10_000),
        "]".repeat(10_000)
    );
    check(deep_json.as_bytes(), "deeply nested JSON extension");
    let mut deep_cbor = vec![
        0x81, 0xa3, 0x00, 0x61, 0x61, 0x02, 0x01, 0x63, b'f', b'o', b'o',
    ];
    deep_cbor.extend(std::iter::repeat_n(0x81, 100_000));
    deep_cbor.push(0x00);
    check(&deep_cbor, "deeply nested CBOR extension");
    // A CBOR array announcing 2^64-1 records, and a byte string announcing 4 GiB.
    check(
        &[0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        "huge CBOR array",
    );
    check(
        &[0x81, 0xa1, 0x08, 0x5a, 0xff, 0xff, 0xff, 0xff],
        "huge CBOR byte string",
    );
    let xml_bomb = r#"<!DOCTYPE sensml [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">]><sensml xmlns="urn:ietf:params:xml:ns:senml"><senml n="&b;" v="1"/></sensml>"#;
    check(xml_bomb.as_bytes(), "XML entity expansion");
    assert!(rsenml::xml::decode(xml_bomb.as_bytes()).is_err());
}
