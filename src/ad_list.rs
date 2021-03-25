use std::ptr::null;

use crate::z_malloc::{z_free, z_malloc_of_type};

pub struct Node<T: Copy + PartialEq> {
    prev: *const Node<T>,
    next: *const Node<T>,
    pub value: T,
}

pub struct List<T: Copy + PartialEq> {
    head: *const Node<T>,
    tail: *const Node<T>,
    len: usize,
    value_clone: Option<fn(T)->T>,
    value_drop: Option<fn(T)>,
    value_equals: Option<fn(T, T)->bool>,
}

enum ItDirection {
    HeadToTail,
    TailToHead,
}

pub struct It<T: Copy + PartialEq> {
    next: *const Node<T>,
    direction: ItDirection,
}

impl<T: Copy + PartialEq> List<T> {
    // same as
    // list *listCreate(void)
    pub fn new() -> Self {
        let list = Self {
            head: null(),
            tail: null(),
            len: 0,
            value_clone: None,
            value_drop: None,
            value_equals: None,
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
            if let Some(value_drop) = self.value_drop {
                value_drop((*current).value);
            }
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

        if let Some(value_drop) = self.value_drop {
            value_drop(node.value);
        }

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
            unsafe {
                if let Some(value_equals) = self.value_equals {
                    if value_equals((*n).value, value) {
                        return n;
                    }
                } else if (*n).value == value {
                    return n;
                }
            }
        }

        null()
    }

    pub fn set_value_clone_method(&mut self, value_clone: Option<fn(T)->T>) {
        self.value_clone = value_clone;
    }

    pub fn set_value_drop_method(&mut self, value_drop: Option<fn(T)>) {
        self.value_drop = value_drop;
    }

    pub fn set_value_equals_method(&mut self, value_equals: Option<fn(T, T)->bool>) {
        self.value_equals = value_equals;
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

impl<T: Copy + PartialEq> Drop for List<T> {
    // same as
    // void listRelease(list *list)
    fn drop(&mut self) {
        unsafe { self.clear(); }
    }
}

impl<T: Copy + PartialEq> Clone for List<T> {
    // same as
    // list *listDup(list *orig)
    fn clone(&self) -> Self {
        let mut copy = Self::new();
        copy.value_clone = self.value_clone;
        copy.value_equals = self.value_equals;
        copy.value_drop = self.value_drop;
        unsafe {
            for n in self.iter() {
                if let Some(value_clone) = self.value_clone {
                    copy.push_back(value_clone((*n).value));
                } else {
                    copy.push_back((*n).value);
                }
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

impl<T: Copy + PartialEq> Iterator for It<T> {
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
