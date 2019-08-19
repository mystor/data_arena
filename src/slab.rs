use crate::source::SlabSource;

use core::alloc::Layout;
use core::mem;
use core::ptr::{self, NonNull};
use core::sync::atomic::AtomicUsize;

#[cfg(feature = "std")]
use core::sync::atomic::Ordering;

#[repr(C)]
pub(crate) struct SlabHeader {
    next: *mut SlabHeader,
    size: usize,
    used: AtomicUsize,
}

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
    Some((
        next,
        NonNull::new_unchecked(start_ptr.add(padding) as *mut u8),
    ))
}

pub(crate) unsafe fn alloc_in_slab_nonatomic(
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

#[cfg(feature = "std")]
pub(crate) unsafe fn alloc_in_slab_atomic(
    slab: *const SlabHeader,
    layout: Layout,
) -> Option<NonNull<u8>> {
    if slab.is_null() {
        return None;
    }

    // Perform a CAS-loop over the `used` field from `SlabHeader`. We can use a
    // relaxed load for reads, as they'll be validated by the
    // compare_exchange_weak, so it's OK to read an out-of-date value. The write
    // doesn't actively protect any memory, so may also be `Relaxed`.
    //
    // XXX(nika): Get someone to double-check that `Relaxed` is OK here.
    let mut prev = (*slab).used.load(Ordering::Relaxed);
    loop {
        let (next, ptr) = alloc_in_slab_common(slab, layout, prev)?;

        match (*slab)
            .used
            .compare_exchange_weak(prev, next, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return Some(ptr),
            Err(next_prev) => prev = next_prev,
        }
    }
}

pub(crate) unsafe fn alloc_slow<S: SlabSource>(
    source: &mut S,
    layout: Layout,
    next: *mut SlabHeader,
) -> Option<(NonNull<SlabHeader>, NonNull<u8>)> {
    // Check if allocation must be larger than the required default size.
    // Required capacity must include the header, the size of the required
    // allocation object, and padding required to align to min_layout's
    // alignment.
    let padding = layout.align().saturating_sub(mem::align_of::<SlabHeader>());
    let min_size = mem::size_of::<SlabHeader>()
        .checked_add(padding)?
        .checked_add(layout.size())?;

    let alloc_layout = Layout::from_size_align(min_size, mem::align_of::<SlabHeader>()).ok()?;

    let (alloc_ptr, size) = source.alloc_slab(alloc_layout);
    assert!(size >= min_size);

    let slab = NonNull::new(alloc_ptr)?.cast::<SlabHeader>();
    let used = AtomicUsize::new(mem::size_of::<SlabHeader>());
    ptr::write(slab.as_ptr(), SlabHeader { next, size, used });

    // As we just allocated our slab, we can do a non-atomic allocation.
    let ptr = alloc_in_slab_nonatomic(slab.as_ptr(), layout)
        .expect("alloc_slab produced insufficiently sized slab");
    Some((slab, ptr))
}

pub(crate) unsafe fn arena_drop<S: SlabSource>(source: &mut S, mut ptr: *mut SlabHeader) {
    while let Some(curr) = NonNull::new(ptr) {
        ptr = curr.as_ref().next;
        let layout =
            Layout::from_size_align_unchecked(curr.as_ref().size, mem::align_of::<SlabHeader>());
        source.dealloc_slab(curr.as_ptr() as *mut u8, layout);
    }
}
