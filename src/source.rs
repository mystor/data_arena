use core::alloc::Layout;

#[cfg(any(feature = "alloc", feature = "std"))]
mod alloc_source;
#[cfg(any(feature = "alloc", feature = "std"))]
pub use alloc_source::AllocSource;

mod buffer_source;
pub use buffer_source::BufferSource;

pub unsafe trait SlabSource {
    /// Allocate a slab which must contain, at a minimum, enough space to
    /// allocate an aligned SlabHeader, followed by the object described by
    /// `Layout`, optionally with padding for alignment.
    unsafe fn alloc_slab(&mut self, min_layout: Layout) -> (*mut u8, usize);

    /// Dealloc a slab which was previously allocated.
    unsafe fn dealloc_slab(&mut self, slab: *mut u8, layout: Layout);
}
