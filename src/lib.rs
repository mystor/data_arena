#![no_std]

mod slab;
pub use slab::SlabSource;

pub mod allocator;

macro_rules! arena_common {
    ($Arena:ident) => {
        impl<'a> $Arena<'a, $crate::allocator::Global> {
            pub fn new() -> Self {
                Self::with_source(Default::default())
            }
        }

        impl<'a, S: $crate::SlabSource> $Arena<'a, S> {
            // ...
        }
    }
}


mod arena;
pub use arena::Arena;

#[cfg(feature = "std")]
mod sync_arena;
#[cfg(feature = "std")]
pub use sync_arena::SyncArena;
