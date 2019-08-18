use alloc::alloc::{Layout};
use crate::SlabSource;
use core::ptr;
use core::cmp;

extern crate alloc;

pub struct Global {
    slab_size: usize,
}

impl Global {
    pub fn new(slab_size: usize) -> Global {
        let slab_size = slab_size.checked_next_power_of_two().unwrap_or(slab_size);
        Global { slab_size }
    }
}

impl Default for Global {
    fn default() -> Self {
        Global {
            slab_size: 4096,
        }
    }
}

unsafe impl SlabSource for Global {
    unsafe fn alloc_slab(&mut self, min_layout: Layout) -> (*mut u8, usize) {
        let size = cmp::max(min_layout.size(), self.slab_size);

        // The alignment of our allocation is always based on `SlabHeader`, even
        // if `Layout` is more-aligned. This allows the `alloc::dealloc` method
        // to be called without storing the alignment of each slab.
        match Layout::from_size_align(size, min_layout.align()) {
            Ok(layout) => (alloc::alloc::alloc(layout), size),
            Err(_) => (ptr::null_mut(), 0),
        }
    }

    unsafe fn dealloc_slab(&mut self, slab: *mut u8, layout: Layout) {
        alloc::alloc::dealloc(slab, layout);
    }
}
