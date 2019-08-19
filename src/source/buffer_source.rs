use crate::source::SlabSource;
use core::alloc::Layout;
use core::ptr;

#[derive(Copy, Clone, Debug)]
pub struct BufferSource<T> {
    buf: T,
    used: bool,
}

impl<T: AsMut<[u8]>> BufferSource<T> {
    pub fn new(buf: T) -> Self {
        BufferSource { buf, used: false }
    }
}

unsafe impl<T: AsMut<[u8]>> SlabSource for BufferSource<T> {
    unsafe fn alloc_slab(&mut self, min_layout: Layout) -> (*mut u8, usize) {
        if self.used {
            return (ptr::null_mut(), 0);
        }

        let buf = self.buf.as_mut();

        // How much of the buffer is avaliable for use?
        let padding = buf.as_mut_ptr().align_offset(min_layout.align());
        let size = if let Some(size) = buf.len().checked_sub(padding) {
            size
        } else {
            return (ptr::null_mut(), 0);
        };

        if size < min_layout.size() {
            return (ptr::null_mut(), 0);
        }

        self.used = true;
        (buf.as_mut_ptr().add(padding), size)
    }

    unsafe fn dealloc_slab(&mut self, _: *mut u8, _: Layout) {
        self.used = false;
    }
}
