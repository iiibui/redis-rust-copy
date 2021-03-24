use std::ptr::null;

use crate::z_malloc::{z_free, z_malloc_of_type};

pub trait Value: Copy + PartialEq {
    fn free(self);
}

macro_rules! plain_value_impl {
    ($t:ty) => {
        impl Value for $t {
            fn free(self) {}
        }
    };
}

plain_value_impl!(i8);
plain_value_impl!(u8);
plain_value_impl!(i16);
plain_value_impl!(u16);
plain_value_impl!(i32);
plain_value_impl!(&i32);
plain_value_impl!(u32);
plain_value_impl!(i64);
plain_value_impl!(u64);
plain_value_impl!(isize);
plain_value_impl!(usize);
plain_value_impl!(f32);
plain_value_impl!(f64);
plain_value_impl!(bool);

pub struct Node<T: Value> {
    prev: *const Node<T>,
    next: *const Node<T>,
    value: T,
}

pub struct List<T: Value> {
    head: *const Node<T>,
    tail: *const Node<T>,
    len: usize,
}

enum ItDirection {
    HeadToTail,
    TailToHead,
}
pub struct It<T: Value> {
    next: *const Node<T>,
    direction: ItDirection,
}

impl<T: Value> List<T> {
    // same as
    // list *listCreate(void)
    pub fn new() -> Self {
        let list = Self {
            head: null(),
            tail: null(),
            len: 0,
        };

        list
    }

    // same as
    // void listEmpty(list *list)
    // unsafe cuz free_value
    pub unsafe fn clear(&mut self) {
        let len = self.len;
        let mut current = self.head;
        for _ in 0..len {
            let next = (*current).next;
            (*current).value.free();
            z_free(current as *const u8);
            current = next;
        }

        self.head = null();
        self.tail = null();
        self.len = 0;
    }

    pub fn push_front(&mut self, value: T) -> &mut Self {
        let node = unsafe { z_malloc_of_type::<Node<T>>() as *mut Node<T> };
        if node.is_null() {
            panic!("z_malloc_of_type fail");
        }

        let node = unsafe { &mut *node };
        node.value = value;
        node.prev = null();

        if self.len == 0 {
            self.head = node;
            self.tail = node;
            node.next = null();
        } else {
            node.next = self.head;
            unsafe { (*(self.head as *mut Node<T>)).prev = node; }
            self.head = node;
        }
        self.len += 1;
        self
    }

    pub fn push_back(&mut self, value: T) -> &mut Self {
        let node = unsafe { z_malloc_of_type::<Node<T>>() as *mut Node<T> };
        if node.is_null() {
            panic!("z_malloc_of_type fail");
        }

        let node = unsafe { &mut *node };
        node.value = value;
        if self.len == 0 {
            self.head = node;
            self.tail = node;
            node.prev = null();
            node.next = null();
        } else {
            node.prev = self.tail;
            node.next = null();
            unsafe { (*(self.tail as *mut Node<T>)).next = node; }
            self.tail = node;
        }
        self.len += 1;
        self
    }

    // same as
    // list *listInsertNode(list *list, listNode *old_node, void *value, int after)
    // unsafe cuz exist_node may be not belong the list
    pub unsafe fn insert_node(&mut self, exist_node: *mut Node<T>, value: T, after: bool) -> &mut Self {
        let node = z_malloc_of_type::<Node<T>>() as *mut Node<T>;
        if node.is_null() {
            panic!("z_malloc_of_type fail");
        }

        let exist_node = &mut *exist_node;
        let node = &mut *node;
        node.value = value;
        if after {
            node.prev = exist_node;
            node.next = exist_node.next;
            if self.tail == exist_node {
                self.tail = node;
            }
        } else {
            node.prev = exist_node.prev;
            node.next = exist_node;
            if self.head == exist_node {
                self.head = node;
            }
        }

        if !node.prev.is_null() {
            (*(node.prev as *mut Node<T>)).next = node;
        }

        if !node.next.is_null() {
            (*(node.next as *mut Node<T>)).prev = node;
        }
        self.len += 1;
        self
    }

    // same as
    // void listDelNode(list *list, listNode *node)
    pub unsafe fn remove(&mut self, node: *mut Node<T>) {
        let node = &mut *node;
        // if prev is null, it is the head node
        if node.prev.is_null() {
            self.head = node.next;
        } else {
            (*(node.prev as *mut Node<T>)).next = node.next;
        }

        // if next is null, it is the tail node
        if node.next.is_null() {
            self.tail = node.prev;
        } else {
            (*(node.next as *mut Node<T>)).prev = node.prev;
        }

        node.value.free();

        z_free(node as *mut Node<T> as *const u8);
        self.len -= 1;
    }

    // same as
    // listNode *listIndex(list *list, long index)
    pub fn get(&self, index: isize) -> *const Node<T> {
        let mut n: *const Node<T>;
        let mut index = index;
        if index < 0 {
            index = (-index) - 1;
            n = self.tail;
            while index > 0 && !n.is_null() {
                unsafe { n = (*n).prev; }
            }
        } else {
            n = self.head;
            while index > 0 && !n.is_null() {
                unsafe { n = (*n).next; }
            }
        }

        n
    }

    // move the tail node to head, same as
    // void listRotateTailToHead(list *list)
    pub fn move_tail_to_head(&mut self) {
        if self.len <= 1 {
            return;
        }

        unsafe {
            let tail = self.tail as *mut Node<T>;
            self.tail = (*tail).prev;
            (*(self.tail as *mut Node<T>)).next = null();

            (*(self.head as *mut Node<T>)).prev = tail;
            (*tail).prev = null();
            (*tail).next = self.head;
            self.head = tail;
        }
    }

    // move the head node to tail, same as
    // void listRotateHeadToTail(list *list)
    pub fn move_head_to_tail(&mut self) {
        if self.len <= 1 {
            return;
        }

        unsafe {
            let head = self.head as *mut Node<T>;
            self.head = (*head).next;
            (*(self.head as *mut Node<T>)).prev = null();

            (*(self.tail as *mut Node<T>)).next = head;
            (*head).next = null();
            (*head).prev = self.tail;
            self.tail = head;
        }
    }

    // after append other is empty, same as
    // void listJoin(list *l, list *o)
    pub fn append(&mut self, other: &mut Self) {
        if other.is_empty() {
            return;
        }

        unsafe {
            (*(other.head as *mut Node<T>)).prev = self.tail;

            // self is empty
            if self.tail.is_null() {
                self.head = other.head;
            } else {
                (*(self.tail as *mut Node<T>)).next = other.head;
            }

            self.tail = other.tail;
            self.len += other.len;

            other.head = null();
            other.tail = null();
            other.len = 0;
        }
    }

    // same as
    // listNode *listSearchKey(list *list, void *key)
    pub fn search(&self, value: T) -> *const Node<T> {
        for n in self.iter() {
                if unsafe { (*n).value } == value {
                    return n;
                }
            }

        null()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn first(&self) -> *const Node<T> {
        self.head
    }

    pub fn last(&self) -> *const Node<T> {
        self.tail
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> It<T> {
        It{next: self.head, direction: ItDirection::HeadToTail}
    }

    pub fn rev_iter(&self) -> It<T> {
        It{next: self.tail, direction: ItDirection::TailToHead}
    }
}

impl<T: Value> Drop for List<T> {
    // same as
    // void listRelease(list *list)
    fn drop(&mut self) {
        unsafe { self.clear(); }
    }
}

impl<T: Value> Clone for List<T> {
    // same as
    // list *listDup(list *orig)
    fn clone(&self) -> Self {
        let mut copy = Self::new();
        unsafe {
            for n in self.iter() {
                copy.push_back((*n).value);
            }
        }

        copy
    }

    fn clone_from(&mut self, source: &Self) {
        unsafe { self.clear(); }
        let dup = source.clone();
        unsafe { std::ptr::write(self, dup); }
    }
}

impl<T: Value> Iterator for It<T> {
    type Item = *const Node<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next;
        if current.is_null() {
            return None;
        }

        match self.direction {
            ItDirection::HeadToTail => {
                unsafe { self.next = (*current).next; }
            }
            ItDirection::TailToHead => {
                unsafe { self.next = (*current).prev; }
            }
        }

        Some(current)
    }
}

#[test]
fn test_list() {
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