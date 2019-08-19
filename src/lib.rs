#![no_std]

mod slab;
pub mod source;

macro_rules! arena_common {
    ($Arena:ident) => {
        #[cfg(any(feature = "alloc", feature = "std"))]
        impl<'a> $Arena<'a, $crate::source::AllocSource> {
            pub fn new() -> Self {
                Self::with_source(Default::default())
            }
        }

        #[cfg(any(feature = "alloc", feature = "std"))]
        impl<'a> Default for $Arena<'a, $crate::source::AllocSource> {
            fn default() -> Self {
                Self::new()
            }
        }

        impl<'a, S: $crate::source::SlabSource> $Arena<'a, S> {
            // ...
            pub fn try_alloc<T: Copy + 'a>(&self, t: T) -> Option<&T> {
                self.try_alloc_no_drop(t)
            }

            pub fn try_alloc_slice<T: Copy + 'a>(&self, t: &[T]) -> Option<&[T]> {
                let layout = Layout::for_value(t);
                unsafe {
                    self.try_alloc_init_no_drop(
                        |ptr| {
                            let src = t.as_ptr();
                            let dst = ptr.cast::<T>().as_ptr();
                            ptr::copy_nonoverlapping(src, dst, t.len());
                            core::slice::from_raw_parts(dst, t.len()).into()
                        },
                        layout,
                    )
                }
            }

            pub fn try_alloc_from_iter<I>(&self, iter: I, len: usize) -> Option<&[I::Item]>
            where
                I: core::iter::IntoIterator,
                I::Item: Copy + 'a,
            {
                self.try_alloc_from_iter_no_drop(iter, len)
            }

            pub fn try_alloc_with<T: Copy + 'a>(&self, f: impl FnOnce() -> T) -> Option<&T> {
                self.try_alloc_with_no_drop(f)
            }

            pub fn try_alloc_no_drop<T: 'a>(&self, t: T) -> Option<&T> {
                self.try_alloc_with_no_drop(|| t)
            }

            pub fn try_alloc_from_iter_no_drop<I>(&self, iter: I, len: usize) -> Option<&[I::Item]>
            where
                I: core::iter::IntoIterator,
                I::Item: 'a,
            {
                let item_layout = Layout::new::<I::Item>();
                let layout = Layout::from_size_align(item_layout.size().checked_mul(len)?, item_layout.align()).ok()?;
                unsafe {
                    self.try_alloc_init_no_drop(
                        |ptr| {
                            let dst = ptr.cast::<I::Item>().as_ptr();

                            let mut src = iter.into_iter();
                            let mut idx = 0;
                            while idx < len {
                                match src.next() {
                                    Some(i) => ptr::write(dst.add(idx), i),
                                    None => break,
                                }
                                idx += 1;
                            }
                            core::slice::from_raw_parts(dst, idx).into()
                        },
                        layout,
                    )
                }
            }

            pub fn try_alloc_with_no_drop<T: 'a>(&self, f: impl FnOnce() -> T) -> Option<&T> {
                unsafe {
                    self.try_alloc_init_no_drop(
                        |p| {
                            ptr::write(p.cast::<T>().as_ptr(), f());
                            p.cast::<T>()
                        },
                        Layout::new::<T>(),
                    )
                }
            }

            pub unsafe fn try_alloc_init_no_drop<T: ?Sized + 'a>(
                &self,
                init: impl FnOnce(NonNull<u8>) -> NonNull<T>,
                layout: Layout,
            ) -> Option<&T> {
                let ptr = self.try_alloc_raw(layout)?;
                Some(&*init(ptr).as_ptr())
            }
        }
    };
}

mod arena;
pub use arena::Arena;

#[cfg(feature = "std")]
mod sync_arena;
#[cfg(feature = "std")]
pub use sync_arena::SyncArena;
