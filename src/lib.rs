#![no_std]

use core::marker::PhantomData;
use core::alloc::Layout;
use core::ptr::{self, NonNull};
use core::cell::{Cell, RefCell};

mod slab;
use slab::{SlabHeader, alloc_in_slab_nonatomic, alloc_slow, arena_drop};

pub mod allocator;
use allocator::{SlabSource, Global};

macro_rules! arena_common {
    ($Arena:ident) => {
        impl<'a> $Arena<'a, Global> {
            pub fn new() -> Self {
                Self::with_source(Default::default())
            }
        }

        impl<'a, S: SlabSource> $Arena<'a, S> {
            // ...
        }
    }
}

#[cfg(feature = "std")]
pub mod sync;

/// An untyped lifecycle-managing arena.
pub struct Arena<'a, S: SlabSource> {
    slab: Cell<*mut SlabHeader>,
    // NOTE: This could _probably_ be an UnsafeCell, with the requirement that
    // SlabSource impls cannot be re-entrant.
    source: RefCell<S>,
    marker: PhantomData<&'a ()>,
}

arena_common!(Arena);

impl<'a, S: SlabSource> Arena<'a, S> {
    pub fn with_source(source: S) -> Self {
        Arena {
            slab: Cell::new(ptr::null_mut()),
            source: RefCell::new(source),
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
        let mut source = self.source.borrow_mut();
        let (slab, ptr) = alloc_slow(&mut *source, layout, old_slab)?;
        self.slab.set(slab.as_ptr());
        Some(ptr)
    }
}

impl<'a, S: SlabSource> Drop for Arena<'a, S> {
    fn drop(&mut self) {
        unsafe {
            arena_drop(self.source.get_mut(), self.slab.get());
        }
    }
}
