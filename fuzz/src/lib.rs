//! Shared checks for the fuzz targets.

use rsenml::MediaType;

/// Decodes `input` as `media` (checked and unchecked). When the checked decode
/// works, the pack must encode again, and encoding is stable: decoding that
/// output and encoding once more gives the same bytes. Comparing bytes (not
/// packs) keeps NaN values from looking like a difference.
pub fn decode_and_round_trip(media: MediaType, input: &[u8]) {
    decode_unchecked(media, input);
    let Ok(pack) = rsenml::decode(media, input) else {
        return;
    };
    let first = match rsenml::encode(media, &pack) {
        Ok(bytes) => bytes,
        Err(e) => panic!("{media:?}: a decoded, valid pack does not encode: {e:?}"),
    };
    let again = match rsenml::decode(media, &first) {
        Ok(pack) => pack,
        Err(e) => panic!("{media:?}: our own output does not decode: {e:?}"),
    };
    let second = match rsenml::encode(media, &again) {
        Ok(bytes) => bytes,
        Err(e) => panic!("{media:?}: re-decoded pack does not encode: {e:?}"),
    };
    assert_eq!(first, second, "{media:?}: encoding is not stable");
}

fn decode_unchecked(media: MediaType, input: &[u8]) {
    match media {
        MediaType::SenmlJson | MediaType::SensmlJson => {
            let _ = rsenml::json::decode_unchecked(input);
        }
        MediaType::SenmlCbor | MediaType::SensmlCbor => {
            let _ = rsenml::cbor::decode_unchecked(input);
        }
        MediaType::SenmlXml | MediaType::SensmlXml => {
            let _ = rsenml::xml::decode_unchecked(input);
        }
        MediaType::SenmlExi | MediaType::SensmlExi => {
            let _ = rsenml::exi::decode_unchecked(input);
        }
    }
}
