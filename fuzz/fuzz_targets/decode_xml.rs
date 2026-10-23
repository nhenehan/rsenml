#![no_main]
//! Every xml decoder on any input: no panic, and valid packs round-trip.

use libfuzzer_sys::fuzz_target;
use rsenml::MediaType;

fuzz_target!(|data: &[u8]| {
    rsenml_fuzz::decode_and_round_trip(MediaType::SenmlXml, data);
    rsenml_fuzz::decode_and_round_trip(MediaType::SensmlXml, data);
});
