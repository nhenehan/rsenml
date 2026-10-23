#![no_main]
//! Every exi decoder on any input: no panic, and valid packs round-trip.

use libfuzzer_sys::fuzz_target;
use rsenml::MediaType;

fuzz_target!(|data: &[u8]| {
    rsenml_fuzz::decode_and_round_trip(MediaType::SenmlExi, data);
    rsenml_fuzz::decode_and_round_trip(MediaType::SensmlExi, data);
});
