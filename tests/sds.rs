use redis_rust_copy::Sds;

#[test]
fn test_sds() {
    let empty1 = Sds::empty();
    let empty2 = Sds::empty();
    assert_eq!(empty1, empty2);

    let s = "get/set";
    let mut sds = Sds::from_str(s);
    assert_eq!(sds.as_str_uncheck(), s);
    assert_ne!(sds, empty1);
    assert!(sds > empty1);
    assert!(sds.starts_with("get".as_bytes()));

    sds.make_ascii_uppercase();
    assert_eq!(sds.as_str_uncheck(), &s.to_ascii_uppercase());

    let s = [1u8, 2, 4, 6];
    let mut sds = Sds::from_slice(&s);
    assert_eq!(sds.as_slice(), &s);
    assert_ne!(sds, empty1);
    assert!(sds > empty1);

    sds.clear();
    assert_eq!(sds, empty1);

    let len = u16::max_value();
    let batch_size = 1024;
    let batch = [9u8].repeat(batch_size);
    let mut total = 0;
    for _ in 0..(len as usize/batch_size) {
        sds.push_slice(batch.as_slice());
        total += batch.len();
    }

    for _ in 0..batch_size {
        sds.push_u8(batch[0]);
        total += 1;
    }

    assert_eq!(sds.len(), total);
    assert_eq!(sds.iter().fold(0u64, |per, item| per + (*item as u64)), total as u64 * 9);
}