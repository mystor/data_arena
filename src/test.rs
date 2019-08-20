extern crate std;

use super::Arena;
use super::source::{AllocSource, SlabSource, InfallibleSource};
use std::mem;
use std::alloc::Layout;
use std::cell::RefCell;
use std::vec::Vec;
use std::ptr::NonNull;

fn check_ptr<T>(val: &T) -> usize {
    let bytep = val as *const _ as *const u8;
    assert_eq!(bytep.align_offset(mem::align_of::<T>()), 0);
    bytep as usize
}

fn check_slice<T>(val: &[T]) -> usize {
    let slicep = val.as_ptr() as *const u8;
    assert_eq!(slicep.align_offset(mem::align_of::<T>()), 0);
    slicep as usize
}

#[test]
fn basic_alloc() {
    let arena = Arena::new();

    let ten = arena.alloc(10u32);
    let twenty = arena.alloc(20u32);
    let byte1 = arena.alloc(b'a');
    let byte2 = arena.alloc(b'b');
    let thirty = arena.alloc(30u32);

    // Values should be correct.
    assert_eq!(ten, &10u32);
    assert_eq!(twenty, &20u32);
    assert_eq!(byte1, &b'a');
    assert_eq!(byte2, &b'b');
    assert_eq!(thirty, &30u32);

    // Values should have correct spacing, and be from the same slab.
    let ten_p = ten as *const _ as usize;
    let twenty_p = twenty as *const _ as usize;
    assert_eq!(twenty_p - ten_p, 4);

    let byte1_p = byte1 as *const _ as usize;
    assert_eq!(byte1_p - twenty_p, 4);

    let byte2_p = byte2 as *const _ as usize;
    assert_eq!(byte2_p - byte1_p, 1);

    let thirty_p = thirty as *const _ as usize;
    assert_eq!(thirty_p - byte2_p, 3);
}

#[test]
fn super_aligned() {
    #[repr(align(32))]
    #[derive(Eq, PartialEq, Debug, Copy, Clone)]
    struct SuperAligned(u8);

    let arena = Arena::new();
    let a = arena.alloc(SuperAligned(b'a'));
    let b = arena.alloc(b'b');
    let ten = arena.alloc(10u32);
    let c = arena.alloc(SuperAligned(b'c'));

    assert_eq!(a, &SuperAligned(b'a'));
    assert_eq!(b, &b'b');
    assert_eq!(ten, &10u32);
    assert_eq!(c, &SuperAligned(b'c'));

    let a_p = check_ptr(a);

    let b_p = check_ptr(b);
    assert_eq!(b_p - a_p, 32);

    let ten_p = check_ptr(ten);
    assert_eq!(ten_p - b_p, 4);

    let c_p = check_ptr(c);
    assert_eq!(c_p - ten_p, 28);
}

#[test]
fn slice() {
    let arena = Arena::new();

    let s0 = arena.alloc_slice(&[5u32, 10, 15, 20]);
    let s1 = arena.alloc_slice(&[1u32, 2, 3, 4]);

    assert_eq!(s0, &[5, 10, 15, 20]);
    assert_eq!(s1, &[1, 2, 3, 4]);

    let s0_p = check_slice(s0);
    let s1_p = check_slice(s1);
    assert_eq!(s1_p - s0_p, 16);
}

const HEADER_SIZE: usize = mem::size_of::<usize>() * 3;

struct TraceSource<'a> {
    source: AllocSource,
    record: &'a RefCell<Vec<(NonNull<u8>, usize)>>,
}

impl<'a> TraceSource<'a> {
    fn new(size: usize, record: &'a RefCell<Vec<(NonNull<u8>, usize)>>) -> Self {
        let source = AllocSource::new(HEADER_SIZE + size);
        TraceSource {source, record }
    }
}

unsafe impl<'a> SlabSource for TraceSource<'a> {
    unsafe fn alloc_slab(&mut self, layout: Layout) -> Option<(NonNull<u8>, usize)> {
        let v = self.source.alloc_slab(layout)?;
        self.record.borrow_mut().push(v);
        Some(v)
    }

    unsafe fn dealloc_slab(&mut self, ptr: NonNull<u8>, layout: Layout) {
        self.record.borrow_mut().retain(|&(old_ptr, size)| {
            if old_ptr == ptr {
                assert_eq!(size, layout.size());
                true
            } else {
                false
            }
        });
        self.source.dealloc_slab(ptr, layout);
    }
}

unsafe impl<'a> InfallibleSource for TraceSource<'a> {
    fn handle_error(layout: Layout) -> ! {
        // Allow the test harness to catch these errors.
        panic!("arena alloc error: {:?}", layout);
    }
}

impl<'a> Drop for TraceSource<'a> {
    fn drop(&mut self) {
        assert!(self.record.borrow().is_empty())
    }
}

#[test]
fn full() {
    let record = RefCell::new(Vec::new());
    let arena = Arena::with_source(TraceSource::new(16, &record));

    let t1 = arena.alloc(10u32);
    assert_eq!(record.borrow().len(), 1);
    let t2 = arena.alloc(20u32);
    assert_eq!(record.borrow().len(), 1);
    let t3 = arena.alloc(30u32);
    assert_eq!(record.borrow().len(), 1);
    let t4 = arena.alloc(40u32);
    assert_eq!(record.borrow().len(), 1);
    let t5 = arena.alloc(50u32);
    assert_eq!(record.borrow().len(), 2);
    let t6 = arena.alloc(60u32);
    assert_eq!(record.borrow().len(), 2);

    assert_eq!(t1, &10);
    assert_eq!(t2, &20);
    assert_eq!(t3, &30);
    assert_eq!(t4, &40);
    assert_eq!(t5, &50);
    assert_eq!(t6, &60);

    let t1_p = check_ptr(t1);
    let t2_p = check_ptr(t2);
    let t3_p = check_ptr(t3);
    let t4_p = check_ptr(t4);
    let t5_p = check_ptr(t5);
    let t6_p = check_ptr(t6);

    assert_eq!(t1_p + 4, t2_p);
    assert_eq!(t2_p + 4, t3_p);
    assert_eq!(t3_p + 4, t4_p);

    // Should start a new page for t5_p
    assert_ne!(t4_p + 4, t5_p);
    assert_eq!(t5_p + 4, t6_p);
}

#[test]
fn large() {
    let record = RefCell::new(Vec::new());
    let arena = Arena::with_source(TraceSource::new(16, &record));

    let t1 = arena.alloc(10u32);
    assert_eq!(record.borrow().len(), 1);

    let t2 = arena.alloc_slice(&[90u8; 512][..]);
    assert_eq!(record.borrow().len(), 2);

    let t3 = arena.alloc(30u32);
    assert_eq!(record.borrow().len(), 3);

    assert_eq!(t1, &10);
    assert_eq!(t2, &[90u8; 512][..]);
    assert_eq!(t3, &30);

    let t1_p = check_ptr(t1);
    let t2_p = check_slice(t2);
    let t3_p = check_ptr(t3);

    assert_ne!(t1_p + 4, t2_p);

    // XXX: Ideally we wouldn't waste the rest of the first allocation if we had
    // to bump into an oversized allocation.
    assert_ne!(t2_p + 512, t3_p);
}
