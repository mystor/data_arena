use alloc::alloc::{Layout};
use core::mem;
use core::ptr::{self, NonNull};
use core::cmp;

use radium::RadiumUsize;

#[cfg(not(feature = "atomic"))]
type CellUsize = core::cell::Cell<usize>;
#[cfg(feature = "atomic")]
type CellUsize = core::sync::atomic::AtomicUsize;

pub unsafe trait Allocator {
    /// Allocate a slab which must contain, at a minimum, enough space to
    /// allocate an aligned SlabHeader, followed by the object described by
    /// `Layout`, optionally with padding for alignment.
    unsafe fn alloc_slab(&self, min_layout: Layout) -> Option<NonNull<SlabHeader>>;

    /// Dealloc a slab which was previously allocated.
    unsafe fn dealloc_slab(&self, slab: NonNull<SlabHeader>);
}

pub struct Global {
    slab_size: usize,
}

impl Global {
    pub fn new(slab_size: usize) -> Global {
        assert!(slab_size > mem::size_of::<SlabHeader>());
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

unsafe impl Allocator for Global {
    unsafe fn alloc_slab(&self, min_layout: Layout) -> Option<NonNull<SlabHeader>> {
        // Check if allocation must be larger than the required default size.
        // Required capacity must include the header, the size of the required
        // allocation object, and padding required to align to min_layout's
        // alignment.
        let required_padding = min_layout.align()
            .saturating_sub(mem::align_of::<SlabHeader>());
        let required_size = mem::size_of::<SlabHeader>()
            .checked_add(required_padding)?
            .checked_add(min_layout.size())?;
        let size = cmp::max(required_size, self.slab_size);

        // The alignment of our allocation is always based on `SlabHeader`, even
        // if `Layout` is more-aligned. This allows the `alloc::dealloc` method
        // to be called without storing the alignment of each slab.
        let layout = Layout::from_size_align(size, mem::align_of::<SlabHeader>()).ok()?;

        let ptr = NonNull::new(alloc::alloc::alloc(layout))?;
        Some(SlabHeader::init(ptr, size))
    }

    unsafe fn dealloc_slab(&self, slab: NonNull<SlabHeader>) {
        let layout = Layout::from_size_align_unchecked(
            slab.as_ref().size(),
            mem::align_of::<SlabHeader>(),
        );
        alloc::alloc::dealloc(slab.as_ptr() as *mut u8, layout);
    }
}

#[repr(C)]
pub struct SlabHeader {
    pub( crate ) next: *mut SlabHeader,
    pub( crate ) size: usize,
    pub( crate ) used: CellUsize,
}

impl SlabHeader {
    pub unsafe fn init(alloc: NonNull<u8>, size: usize) -> NonNull<SlabHeader> {
        let alloc = alloc.cast::<SlabHeader>();
        ptr::write(
            alloc.as_ptr(),
            SlabHeader {
                next: ptr::null_mut(),
                size,
                used: RadiumUsize::new(mem::size_of::<SlabHeader>()),
            },
        );
        alloc
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

