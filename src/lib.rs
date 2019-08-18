#![no_std]

use core::marker::PhantomData;
use core::alloc::Layout;
use core::ptr::{self, NonNull};
use core::cell::Cell;

mod slab;
use slab::{SlabHeader, alloc_in_slab_nonatomic, alloc_slow, arena_drop};

pub mod allocator;
use allocator::{Allocator, Global};

macro_rules! arena_common {
    ($Arena:ident) => {
        impl<'a> $Arena<'a, Global> {
            pub fn new() -> Self {
                Self::with_alloc(Default::default())
            }
        }

        impl<'a, A: Allocator> $Arena<'a, A> {
            // ...
        }
    }
}

#[cfg(feature = "std")]
pub mod sync;

/// An untyped lifecycle-managing arena.
pub struct Arena<'a, A: Allocator> {
    slab: Cell<*mut SlabHeader>,
    alloc: A,
    marker: PhantomData<&'a ()>,
}

arena_common!(Arena);

impl<'a, A: Allocator> Arena<'a, A> {
    pub fn with_alloc(alloc: A) -> Self {
        Arena {
            slab: Cell::new(ptr::null_mut()),
            alloc,
            marker: PhantomData
        }
    }

    pub unsafe fn try_alloc_raw(&self, layout: Layout) -> Option<NonNull<u8>> {
        let slab = self.slab.get();
        if let Some(ptr) = alloc_in_slab_nonatomic(slab, layout) {
            return Some(ptr);
        }

        self.try_alloc_raw_slow(layout, slab)
    }

    #[inline(never)]
    unsafe fn try_alloc_raw_slow(&self, layout: Layout, old_slab: *mut SlabHeader) -> Option<NonNull<u8>> {
        let (slab, ptr) = alloc_slow(&self.alloc, layout, old_slab)?;
        self.slab.set(slab.as_ptr());
        Some(ptr)
    }
}

impl<'a, A: Allocator> Drop for Arena<'a, A> {
    fn drop(&mut self) {
        unsafe {
            arena_drop(&self.alloc, self.slab.get());
        }
    }
}
