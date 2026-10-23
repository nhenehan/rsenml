//! A bump allocator over a fixed 32 KiB static arena: `dealloc` frees nothing,
//! `alloc` returns null when the arena is exhausted. It is the smallest
//! allocator that satisfies `alloc`, so `.text` measures the codec, not malloc.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

const ARENA_SIZE: usize = 32 * 1024;

struct Arena(core::cell::UnsafeCell<[u8; ARENA_SIZE]>);
// SAFETY: bytes are handed out once each, from a monotonically increasing offset.
unsafe impl Sync for Arena {}

static ARENA: Arena = Arena(core::cell::UnsafeCell::new([0; ARENA_SIZE]));
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Bump;

unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = ARENA.0.get() as usize;
        let mut next = NEXT.load(Ordering::Relaxed);
        loop {
            let start = (base + next).next_multiple_of(layout.align()) - base;
            let Some(end) = start.checked_add(layout.size()).filter(|&e| e <= ARENA_SIZE) else {
                return core::ptr::null_mut();
            };
            match NEXT.compare_exchange_weak(next, end, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return (base + start) as *mut u8,
                Err(seen) => next = seen,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static GLOBAL: Bump = Bump;
