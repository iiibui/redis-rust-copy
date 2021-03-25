use redis_rust_copy::{List, Node};
use redis_rust_copy::{z_malloc_of_type, z_free};

#[test]
fn test_basic() {
    let mut list = List::new();
    assert!(list.is_empty());

    list.push_front(1);
    assert!(!list.is_empty());
    unsafe {
        assert_eq!((*list.first()).value, 1);
    }

    list.push_back(2);
    unsafe {
        assert_eq!((*list.last()).value, 2);
        assert_eq!(list.len(), 2);
    }

    let elements: Vec<_> = list.iter()
        .map(|n| unsafe{(*n).value})
        .collect();
    assert_eq!(elements.as_slice(), &[1, 2]);

    unsafe {
        assert_eq!((*list.get(0)).value, 1);
        assert_eq!((*list.get(-1)).value, 2);
        assert!(list.get(2).is_null());
        assert!(list.get(-3).is_null());
    }

    list.move_head_to_tail();
    unsafe {
        assert_eq!((*list.first()).value, 2);
        assert_eq!((*list.last()).value, 1);
    }

    list.move_tail_to_head();
    unsafe {
        assert_eq!((*list.first()).value, 1);
        assert_eq!((*list.last()).value, 2);
    }

    let mut other = list.clone();
    other.move_tail_to_head();
    list.push_back(3).append(&mut other);
    assert!(other.is_empty());

    let elements: Vec<_> = list.iter()
        .map(|n| unsafe{(*n).value})
        .collect();
    assert_eq!(elements.as_slice(), &[1, 2, 3, 2, 1]);

    unsafe { list.remove(list.search(3) as *mut Node<_>); }
    let elements: Vec<_> = list.iter()
        .map(|n| unsafe{(*n).value})
        .collect();
    assert_eq!(elements.as_slice(), &[1, 2, 2, 1]);

    list.move_head_to_tail();
    let elements: Vec<_> = list.rev_iter()
        .map(|n| unsafe{(*n).value})
        .collect();
    assert_eq!(elements.as_slice(), &[1, 1, 2, 2]);

    unsafe {
        list.insert_node(list.first() as *mut Node<_>, 3, false);
        list.insert_node(list.last() as *mut Node<_>, 3, true);
    }
    let elements: Vec<_> = list.rev_iter()
        .map(|n| unsafe{(*n).value})
        .collect();
    assert_eq!(elements.as_slice(), &[3, 1, 1, 2, 2, 3]);
}

struct Data {
    size: isize,
}

#[test]
fn test_custom() {
    unsafe {
        let mut list = List::new();
        list.set_value_drop_method(Some(|v| z_free(v as *const u8)));
        let p = z_malloc_of_type::<Data>();
        list.push_back(p);
        (*(p as *mut Data)).size = 123;
        for n in list.iter() {
            assert_eq!((*n).value, p);
        }
    }
}