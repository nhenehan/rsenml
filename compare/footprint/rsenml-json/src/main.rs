//! rsenml, `json` feature only: decode (and validate) the second example of
//! RFC 8428 section 5.1.2, re-encode it. Heap: the bump allocator of `common/bump.rs`.
#![no_std]
#![no_main]

#[path = "../../common/bump.rs"]
mod bump;
#[path = "../../common/rt.rs"]
mod rt;

static INPUT: &[u8] = br#"[
{"bn":"urn:dev:ow:10e2073a0108006:","bt":1.276020076001e+09,"bu":"A","bver":5,"n":"voltage","u":"V","v":120.1},
{"n":"current","t":-5,"v":1.2},{"n":"current","t":-4,"v":1.3},{"n":"current","t":-3,"v":1.4},
{"n":"current","t":-2,"v":1.5},{"n":"current","t":-1,"v":1.6},{"n":"current","v":1.7}
]"#;

fn work(input: &[u8]) -> usize {
    match rsenml::json::decode(input).and_then(|pack| rsenml::json::encode(&pack)) {
        Ok(bytes) => bytes.len(),
        Err(_) => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    rt::run(INPUT, work)
}
