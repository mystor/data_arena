//! # Untyped Arenas for Data Types
//! An arena - a fast, but limited allocator.
//!
//! This module defines the [`Arena`], a

#![no_std]

mod slab;
pub mod source;

macro_rules! arena_common {
    ($Arena:ident) => {
        #[cfg(any(feature = "alloc", feature = "std"))]
        impl<'a> $Arena<'a, $crate::source::AllocSource> {
            /// Create a new Arena with the default allocation strategy.
            pub fn new() -> Self {
                Self::with_source(Default::default())
            }
        }

        impl<'a, S: $crate::source::SlabSource + Default> Default for $Arena<'a, S> {
            fn default() -> Self {
                Self::with_source(Default::default())
            }
        }

        impl<'a, S: $crate::source::SlabSource> $Arena<'a, S> {
            pub fn with_source(source: S) -> Self {
                $Arena {
                    slab: Default::default(),
                    source: source.into(),
                    marker: PhantomData,
                }
            }
        }

        impl<'a, S: $crate::source::InfallibleSource> $Arena<'a, S> {
            pub fn alloc<T: Copy + 'a>(&self, t: T) -> &T {
                self.alloc_no_drop(t)
            }

            pub fn alloc_slice<'s, T: Copy + 'a>(&'s self, t: &[T]) -> &'s [T] {
                S::unwrap(self.try_alloc_slice(t), || Layout::for_value(t))
            }

            pub fn alloc_from_iter<I>(&self, iter: I, len: usize) -> &[I::Item]
            where
                I: core::iter::IntoIterator,
                I::Item: Copy + 'a,
            {
                self.alloc_from_iter_no_drop(iter, len)
            }

            pub fn alloc_with<T: Copy + 'a>(&self, f: impl FnOnce() -> T) -> &T {
                self.alloc_with_no_drop(f)
            }

            pub fn alloc_no_drop<T: 'a>(&self, t: T) -> &T {
                S::unwrap(self.try_alloc_no_drop(t), || Layout::new::<T>())
            }

            pub fn alloc_from_iter_no_drop<I>(&self, iter: I, len: usize) -> &[I::Item]
            where
                I: core::iter::IntoIterator,
                I::Item: 'a,
            {
                S::unwrap(self.try_alloc_from_iter_no_drop(iter, len), || {
                    // Re-calculate the layout which was to be allocated to
                    // give to the alloc failure reporter.
                    let item_layout = Layout::new::<I::Item>();
                    let size = item_layout.size().saturating_mul(len);
                    Layout::from_size_align(size, item_layout.align()).unwrap_or(item_layout)
                })
            }

            pub fn alloc_with_no_drop<T: 'a>(&self, f: impl FnOnce() -> T) -> &T {
                S::unwrap(self.try_alloc_with_no_drop(f), || Layout::new::<T>())
            }

            pub unsafe fn alloc_init_no_drop<T: ?Sized + 'a>(
                &self,
                init: impl FnOnce(NonNull<u8>) -> NonNull<T>,
                layout: Layout,
            ) -> &T {
                S::unwrap(self.try_alloc_init_no_drop(init, layout), || layout)
            }

            pub unsafe fn alloc_raw(&self, layout: Layout) -> NonNull<u8> {
                S::unwrap(self.try_alloc_raw(layout), || layout)
            }
        }

        impl<'a, S: $crate::source::SlabSource> $Arena<'a, S> {
            pub fn try_alloc<T: Copy + 'a>(&self, t: T) -> Option<&T> {
                self.try_alloc_no_drop(t)
            }

            pub fn try_alloc_slice<'s, T: Copy + 'a>(&'s self, t: &[T]) -> Option<&'s [T]> {
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
                let size = item_layout.size().checked_mul(len)?;
                let layout = Layout::from_size_align(size, item_layout.align()).ok()?;
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
