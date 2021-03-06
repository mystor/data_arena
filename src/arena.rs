use crate::slab::{alloc_in_slab_nonatomic, alloc_slow, arena_drop, SlabHeader};
use crate::source::SlabSource;

use core::alloc::Layout;
use core::cell::{Cell, RefCell};
use core::marker::PhantomData;
use core::ptr::{self, NonNull};

/// An untyped lifecycle-managing arena.
pub struct Arena<'a, S: SlabSource> {
    slab: Cell<Option<NonNull<SlabHeader>>>,
    // NOTE: This could _probably_ be an UnsafeCell, with the requirement that
    // SlabSource impls cannot be re-entrant.
    source: RefCell<S>,
    marker: PhantomData<&'a ()>,
}

arena_common!(Arena);

impl<'a, S: SlabSource> Arena<'a, S> {
    pub unsafe fn try_alloc_raw(&self, layout: Layout) -> Option<NonNull<u8>> {
        let slab = self.slab.get();
        if let Some(ptr) = alloc_in_slab_nonatomic(slab, layout) {
            return Some(ptr);
        }

        self.try_alloc_raw_slow(layout, slab)
    }

    #[inline(never)]
    unsafe fn try_alloc_raw_slow(
        &self,
        layout: Layout,
        old_slab: Option<NonNull<SlabHeader>>,
    ) -> Option<NonNull<u8>> {
        let mut source = self.source.borrow_mut();
        let (slab, ptr) = alloc_slow(&mut *source, layout, old_slab)?;
        self.slab.set(Some(slab));
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
