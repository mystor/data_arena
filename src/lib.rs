#![no_std]

extern crate alloc;

use alloc::alloc::Layout;

use core::marker::PhantomData;
use core::ptr::{self, NonNull};
use core::sync::atomic;

use radium::{RadiumPtr, RadiumUsize};

pub mod allocator;
use allocator::{Allocator, Global, SlabHeader};

#[cfg(not(feature = "atomic"))]
use core::cell::Cell;
#[cfg(not(feature = "atomic"))]
type CellPtr<T> = Cell<*mut T>;
#[cfg(feature = "atomic")]
type CellPtr<T> = atomic::AtomicPtr<T>;


unsafe fn do_alloc_in_slab(
    slab: *mut SlabHeader,
    layout: Layout,
) -> Option<*mut u8> {
    // We need a slab to allocate anything.
    if slab.is_null() {
        return None;
    }

    // Perform a CAS-loop over the `used` field from `SlabHeader`. We can use a
    // relaxed load for reads, as they'll be validated by the
    // compare_exchange_weak, so it's OK to read an out-of-date value. The write
    // doesn't actively protect any memory, so may also be `Relaxed`.
    //
    // XXX(nika): Get someone to double-check that `Relaxed` is OK here.
    let mut prev = RadiumUsize::load(&(*slab).used, atomic::Ordering::Relaxed);
    loop {
        // Current value for the allocation head.
        let start_ptr = (slab as *mut u8).add(prev);

        // Amount of padding needed to align the current pointer to the
        // required allocation alignment.
        let padding = start_ptr.align_offset(layout.align());

        // Determine the value after the end of the new allocation.
        let next = prev
            .checked_add(padding)?
            .checked_add(layout.size())?;

        if next > (*slab).size {
            return None;
        }

        match RadiumUsize::compare_exchange_weak(
            &(*slab).used,
            prev,
            next,
            atomic::Ordering::Relaxed,
            atomic::Ordering::Relaxed,
        ) {
            Ok(_) => return Some(start_ptr.add(padding)),
            Err(next_prev) => prev = next_prev,
        }
    }
}

#[inline(never)]
unsafe fn do_alloc_slow<A: Allocator>(
    arena: &Arena<'_, A>,
    layout: Layout,
    mut prev: *mut SlabHeader,
) -> Option<*mut u8> {
    let mut new_slab = arena.alloc.alloc_slab(layout)?;
    let alloc = do_alloc_in_slab(new_slab.as_ptr(), layout)
        .expect("alloc_slab produced insufficiently sized/aligned slab");

    loop {
        new_slab.as_mut().next = prev;
        match arena.slab.compare_exchange_weak(
            prev,
            new_slab.as_ptr(),
            atomic::Ordering::Relaxed,
            atomic::Ordering::Relaxed,
        ) {
            Ok(_) => return Some(alloc),
            Err(next_prev) => {
                // If we've gotten a new slab from another thread, check if we
                // can allocate into it.
                if let Some(ptr) = do_alloc_in_slab(prev, layout) {
                    // Free the slab we allocated unnecessarially.
                    arena.alloc.dealloc_slab(new_slab);
                    return Some(ptr);
                }

                prev = next_prev;
            }
        }
    }
}

unsafe fn do_alloc<A: Allocator>(
    arena: &Arena<'_, A>,
    layout: Layout,
) -> Option<*mut u8> {
    let prev = arena.slab.load(atomic::Ordering::Relaxed);
    if let Some(ptr) = do_alloc_in_slab(prev, layout) {
        return Some(ptr);
    }

    do_alloc_slow(arena, layout, prev)
}

/// An untyped lifecycle-managing arena.
pub struct Arena<'a, A>
where
    A: Allocator,
{
    slab: CellPtr<SlabHeader>,
    alloc: A,
    marker: PhantomData<&'a ()>,
}

impl<'a> Arena<'a, Global> {
    pub fn new() -> Self {
        Self::with_alloc(Default::default())
    }
}

impl<'a, A> Arena<'a, A>
where
    A: Allocator,
{
    pub fn with_alloc(alloc: A) -> Self {
        Arena {
            slab: RadiumPtr::new(ptr::null_mut()),
            alloc,
            marker: PhantomData,
        }
    }

    pub fn alloc<T>(&self, t: T) -> &T
    where
        T: Copy + 'a,
    {
        self.alloc_nodrop(t)
    }

    pub fn alloc_nodrop<T>(&self, t: T) -> &T
    where
        T: 'a,
    {
        self.alloc_with_nodrop(|| t)
    }

    pub fn alloc_with_nodrop<F, T>(&self, init: F) -> &T
    where
        F: FnOnce() -> T,
        T: 'a,
    {
        unsafe {
            self.alloc_inplace(|p| {
                ptr::write(p as *mut T, init());
                p as *mut T
            }, Layout::new::<T>())
        }
    }

    unsafe fn alloc_inplace<F, T>(&self, init: F, layout: Layout) -> &T
    where
        F: FnOnce(*mut u8) -> *mut T,
        T: 'a+ ?Sized,
    {
        match do_alloc(self, layout) {
            Some(ptr) => &*init(ptr),
            None => alloc::alloc::handle_alloc_error(layout),
        }
    }
}


impl<'arena, A> Drop for Arena<'arena, A>
where
    A: Allocator,
{
    fn drop(&mut self) {
        let mut ptr = self.slab.load(atomic::Ordering::Acquire);
        while let Some(curr) = NonNull::new(ptr) {
            unsafe {
                ptr = curr.as_ref().next;
                self.alloc.dealloc_slab(curr);
            }
        }
    }
}
