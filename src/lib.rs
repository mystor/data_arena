#![no_std]

extern crate alloc;

use core::marker::PhantomData;
use core::alloc::Layout;
use core::ptr::{self, NonNull};
use core::cell::Cell;

pub mod allocator;
use allocator::{Allocator, Global, SlabHeader};

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

unsafe fn alloc_in_slab_common(
    slab: *const SlabHeader,
    layout: Layout,
    used: usize,
) -> Option<(usize, NonNull<u8>)> {
    // Current value for the allocation head.
    let start_ptr = (slab as *const u8).add(used);

    // Amount of padding needed to align the current pointer to the
    // required allocation alignment.
    let padding = start_ptr.align_offset(layout.align());

    // Determine the value after the end of the new allocation.
    let next = used.checked_add(padding)?.checked_add(layout.size())?;

    if next > (*slab).size {
        return None;
    }
    Some((next, NonNull::new_unchecked(start_ptr.add(padding) as *mut u8)))
}

unsafe fn alloc_in_slab_nonatomic(
    slab: *mut SlabHeader,
    layout: Layout,
) -> Option<NonNull<u8>> {
    if slab.is_null() {
        return None;
    }

    // When non-atomic, this method has exclusive access to the slab header. Use
    // this access to perform optimizable non-atomic loads.
    let prev = *(*slab).used.get_mut();
    let (next, ptr) = alloc_in_slab_common(slab, layout, prev)?;
    *(*slab).used.get_mut() = next;
    Some(ptr)
}

unsafe fn alloc_slow<A: Allocator>(
    alloc: &A,
    layout: Layout,
) -> Option<(NonNull<SlabHeader>, NonNull<u8>)> {
    let slab = alloc.alloc_slab(layout)?;

    // As we just allocated our slab, we can do a non-atomic allocation.
    let ptr = alloc_in_slab_nonatomic(slab.as_ptr(), layout)
        .expect("alloc_slab produced insufficiently sized slab");
    Some((slab, ptr))
}

unsafe fn arena_drop<A: Allocator>(
    alloc: &A,
    mut ptr: *mut SlabHeader,
) {
    while let Some(curr) = NonNull::new(ptr) {
        ptr = curr.as_ref().next;
        alloc.dealloc_slab(curr);
    }
}

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
        let (mut slab, ptr) = alloc_slow(&self.alloc, layout)?;
        slab.as_mut().next = old_slab;
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
