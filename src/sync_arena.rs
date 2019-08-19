use crate::slab::{alloc_in_slab_atomic, alloc_slow, arena_drop, SlabHeader};
use crate::source::SlabSource;

extern crate std;
use std::sync::{LockResult, Mutex};

use core::alloc::Layout;
use core::marker::PhantomData;
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicPtr, Ordering};

fn ignore_poison<T>(result: LockResult<T>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    }
}

/// A threadsafe untyped lifecycle-managing arena.
///
/// *This type is only available when built with the `std` feature*
pub struct SyncArena<'a, S: SlabSource> {
    slab: AtomicPtr<SlabHeader>,
    source: Mutex<S>,
    marker: PhantomData<&'a ()>,
}

arena_common!(SyncArena);

impl<'a, S: SlabSource> SyncArena<'a, S> {
    pub fn with_source(source: S) -> Self {
        SyncArena {
            slab: AtomicPtr::new(ptr::null_mut()),
            source: Mutex::new(source),
            marker: PhantomData,
        }
    }

    pub unsafe fn try_alloc_raw(&self, layout: Layout) -> Option<NonNull<u8>> {
        let slab = self.slab.load(Ordering::Relaxed);
        if let Some(ptr) = alloc_in_slab_atomic(slab, layout) {
            return Some(ptr);
        }

        self.try_alloc_raw_slow(layout, slab)
    }

    #[inline(never)]
    unsafe fn try_alloc_raw_slow(
        &self,
        layout: Layout,
        orig_slab: *mut SlabHeader,
    ) -> Option<NonNull<u8>> {
        // Acquire the slab source lock. After this has been acquired, the
        // `slab` member cannot be changed by another thread.
        let mut source_guard = ignore_poison(self.source.lock());

        // Check if the slab value has changed since the last time it was read.
        // If it has, try to allocate in the new slab.
        //
        // XXX: This uses `Acquire` ordering to match with the release ordering
        // used to update the `slab` ptr at the end of this method. I am unsure
        // whether `Mutex` lock/unlock is sufficient to provide the ordering I
        // need here.
        let old_slab = self.slab.load(Ordering::Acquire);
        if old_slab != orig_slab {
            if let Some(ptr) = alloc_in_slab_atomic(old_slab, layout) {
                return Some(ptr);
            }
        }

        // A new allocation is needed. Perform the allocation and add it to the
        // front of the list.
        let (slab, ptr) = alloc_slow(&mut *source_guard, layout, old_slab)?;

        // This store is OK, as no thread will write to `self.slab` without
        // holding the alloc lock.
        //
        // XXX: A CAS-loop could be used instead of the `alloc_guard` if
        // `no_std` support is desired.
        self.slab.store(slab.as_ptr(), Ordering::Release);
        Some(ptr)
    }
}

impl<'a, S: SlabSource> Drop for SyncArena<'a, S> {
    fn drop(&mut self) {
        // XXX: Not sure if I need to fence here to make sure this thread has
        // seem atomic loads/stores from other threads?
        unsafe {
            arena_drop(ignore_poison(self.source.get_mut()), *self.slab.get_mut());
        }
    }
}
