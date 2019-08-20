use super::Arena;

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

    let a_p = a as *const _ as usize;
    assert_eq!((a_p as *const u8).align_offset(32), 0);

    let b_p = b as *const _ as usize;
    assert_eq!((b_p as *const u8).align_offset(1), 0);
    assert_eq!(b_p - a_p, 32);

    let ten_p = ten as *const _ as usize;
    assert_eq!((ten_p as *const u8).align_offset(4), 0);
    assert_eq!(ten_p - b_p, 4);

    let c_p = c as *const _ as usize;
    assert_eq!((c_p as *const u8).align_offset(32), 0);
    assert_eq!(c_p - ten_p, 28);
}
