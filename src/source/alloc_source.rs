use crate::source::{InfallibleSource, SlabSource};
use core::alloc::Layout;
use core::cmp;
use core::ptr::NonNull;

extern crate alloc;

#[derive(Copy, Clone, Debug)]
pub struct AllocSource {
    slab_size: usize,
}

impl AllocSource {
    pub fn new(slab_size: usize) -> AllocSource {
        AllocSource { slab_size }
    }
}

impl Default for AllocSource {
    fn default() -> Self {
        AllocSource { slab_size: 4096 }
    }
}

unsafe impl SlabSource for AllocSource {
    unsafe fn alloc_slab(&mut self, min_layout: Layout) -> Option<(NonNull<u8>, usize)> {
        let size = cmp::max(min_layout.size(), self.slab_size);

        // The alignment of our allocation is always based on `SlabHeader`, even
        // if `Layout` is more-aligned. This allows the `alloc::dealloc` method
        // to be called without storing the alignment of each slab.
        let layout = Layout::from_size_align(size, min_layout.align()).ok()?;
        let ptr = NonNull::new(alloc::alloc::alloc(layout))?;
        Some((ptr, size))
    }

    unsafe fn dealloc_slab(&mut self, slab: NonNull<u8>, layout: Layout) {
        alloc::alloc::dealloc(slab.as_ptr(), layout);
    }
}

unsafe impl InfallibleSource for AllocSource {
    fn handle_error(layout: Layout) -> ! {
        alloc::alloc::handle_alloc_error(layout)
    }
}
