//! Minimal bare-metal runtime shared by the footprint binaries: an entry point
//! (the linker's GC root), a panic handler that spins, and a sink that keeps
//! the result alive.

use core::sync::atomic::{AtomicUsize, Ordering};

/// Written by `_start` so the work cannot be optimised away.
pub static SINK: AtomicUsize = AtomicUsize::new(0);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Runs `work` forever on an opaque copy of `input`.
pub fn run(input: &'static [u8], work: fn(&[u8]) -> usize) -> ! {
    loop {
        let input = core::hint::black_box(input);
        SINK.store(work(input), Ordering::Relaxed);
    }
}
