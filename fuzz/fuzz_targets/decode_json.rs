#![no_main]
//! Every json decoder on any input: no panic, and valid packs round-trip.

use libfuzzer_sys::fuzz_target;
use rsenml::MediaType;

fuzz_target!(|data: &[u8]| {
    rsenml_fuzz::decode_and_round_trip(MediaType::SenmlJson, data);
    rsenml_fuzz::decode_and_round_trip(MediaType::SensmlJson, data);
});
