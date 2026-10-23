//! Baseline: the runtime and the bump allocator, one heap allocation, no codec.
#![no_std]
#![no_main]

extern crate alloc;

#[path = "../../common/bump.rs"]
mod bump;
#[path = "../../common/rt.rs"]
mod rt;

static INPUT: &[u8] = b"baseline";

fn work(input: &[u8]) -> usize {
    core::hint::black_box(alloc::vec::Vec::from(input)).len()
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    rt::run(INPUT, work)
}
