use std::mem::size_of;
use crate::int_set::Encoding::{INT64, INT16, INT32};
use crate::{z_malloc_usable, z_realloc_usable, z_free};
use std::fmt::{Display, Formatter, Debug};

#[derive(PartialOrd, PartialEq, Copy, Clone, Debug)]
enum Encoding {
    INT16 = size_of::<i16>() as isize,
    INT32 = size_of::<i32>() as isize,
    INT64 = size_of::<i64>() as isize,
}

impl Encoding {
    // like
    // uint8_t _intsetValueEncoding(int64_t v)
    fn value_encoding(v: i64) -> Self {
        if v < i32::MIN as i64 || v > i32::MAX as i64 {
            INT64
        } else if v < i16::MIN as i64 || v > i16::MAX as i64 {
            INT32
        } else {
            INT16
        }
    }

    // byte size of the encoding, same as the enum value
    #[inline]
    fn byte_size(self) -> usize {
        self as usize
    }
}

pub struct IntSet(*const IntSetInner);

#[repr(C)]
struct IntSetInner {
    encoding: Encoding,
    len: u16,
    alloc: u16,
    contents: [u8;0],
}

static EMPTY_SET: IntSetInner = IntSetInner {
    encoding: Encoding::INT16,
    len: 0,
    alloc: 0,
    contents: []
};

impl IntSet {
    pub fn new() -> Self {
        Self(&EMPTY_SET as *const IntSetInner)
    }

    #[inline]
    fn is_global_empty(&self) -> bool {
        self.0 == (&EMPTY_SET as *const IntSetInner)
    }

    #[inline]
    fn inner_ref(&self) -> &IntSetInner {
        unsafe {&*(self.0)}
    }

    #[inline]
    fn inner_mut_ref(&mut self) -> &mut IntSetInner {
        unsafe {&mut *(self.0 as *mut IntSetInner)}
    }

    // like
    // intset *intsetResize(intset *is, uint32_t len)
    fn resize(&mut self, len: u16) {
        let mut inner = self.inner_mut_ref();
        let mut size = len as usize * inner.encoding.byte_size();
        if size > u16::MAX as usize {
            panic!("oversize: {} > {}", size, u16::MAX);
        }
        if size > inner.alloc as usize {
            size  += size_of::<IntSetInner>();
            unsafe {
                let (ptr, usable) = if self.is_global_empty() {
                    let (ptr, usable) = z_malloc_usable(size as usize);
                    self.0.copy_to_nonoverlapping(ptr as *mut IntSetInner, 1);
                    (ptr, usable)
                } else {
                    z_realloc_usable(self.0 as *const u8, size)
                };
                self.0 = ptr as *const IntSetInner;
                inner = self.inner_mut_ref();
                inner.alloc = (usable - size_of::<IntSetInner>()) as u16;
            }
        }
    }

    // like
    // static uint8_t intsetSearch(intset *is, int64_t value, uint32_t *pos)
    // but first return is if found, second is pos and check encoding before
    fn search(&self, value: i64) -> (bool, isize) {
        let inner = self.inner_ref();
        if inner.len == 0 {
            (false, 0)
        } else {
            unsafe {
                match inner.encoding {
                    INT16 => self.typed_search::<i16>(value as i16),
                    INT32 => self.typed_search::<i32>(value as i32),
                    INT64 => self.typed_search::<i64>(value as i64),
                }
            }
        }
    }

    unsafe fn typed_search<T: Copy + PartialOrd>(&self, value: T) -> (bool, isize) {
        let inner = self.inner_ref();
        let contents = inner.contents.as_ptr() as *const T;
        let mut min = 0isize;
        let mut max = inner.len as isize - 1;
        if value > *contents.offset(max) {
            (false, inner.len as isize)
        } else if value < *contents {
            (false, 0)
        } else {
            let mut mid = 0;
            while max >= min {
                mid = (min + max) >> 1;
                let mid_value = *contents.offset(mid);
                if value > mid_value {
                    min = mid + 1;
                } else if value < mid_value {
                    max = mid - 1;
                } else {
                    return (true, mid);
                }
            }
            (false, mid)
        }
    }

    // like
    // static intset *intsetUpgradeAndAdd(intset *is, int64_t value)
    // value only too max or too min need upgrade, too min mean < 0
    fn upgrade_and_add(&mut self, value: i64, value_enc: Encoding) -> bool {
        let inner = self.inner_mut_ref();
        let current_enc = inner.encoding;
        let current_len = inner.len;

        inner.encoding = value_enc;
        self.resize(current_len + 1);

        // resize may realloc so must re ref or rust will complain
        let inner = self.inner_mut_ref();
        let prepend = value < 0;

        unsafe {
            match current_enc {
                INT16 => match value_enc {
                    INT16 => unreachable!("int16=>int16"),
                    INT32 => {
                        let from = inner.contents.as_ptr() as *mut i16;
                        let to = from as *mut i32;
                        IntSet::move_one_by_one_then_put(from, current_len as isize, to, value as i32, prepend);
                    }
                    INT64 => {
                        let from = inner.contents.as_ptr() as *mut i16;
                        let to = from as *mut i64;
                        IntSet::move_one_by_one_then_put(from, current_len as isize, to, value, prepend);
                    }
                }
                INT32 => match value_enc {
                    INT16 => unreachable!("int32=>int16"),
                    INT32 => unreachable!("int32=>int32"),
                    INT64 => {
                        let from = inner.contents.as_ptr() as *mut i32;
                        let to = from as *mut i64;
                        IntSet::move_one_by_one_then_put(from, current_len as isize, to, value, prepend);
                    }
                }
                INT64 => unreachable!("int64=>"),
            }
        }

        inner.len += 1;

        return true;
    }

    unsafe fn move_one_by_one_then_put<F,T>(from: *mut F, size: isize, to: *mut T, value: T, prepend: bool)
        where F: Copy, T:Copy + From<F> {
        if prepend {
            // prepend, end to start to avoid overlap and every element move one pos
            for i in (0..size).rev() {
                *to.offset(i + 1) = (*from.offset(i)).into();
            }
            *to = value;
        } else {
            // append, end to start to avoid overlap
            for i in (0..size).rev() {
                *to.offset(i) = (*from.offset(i)).into();
            }
            *to.offset(size) = value;
        }
    }

    #[inline]
    unsafe fn put_one<T>(&mut self, pos: isize, value: T, need_move: bool) {
        let inner = self.inner_mut_ref();
        let from = (inner.contents.as_ptr() as *mut T).offset(pos);
        if need_move {
            let count = inner.len as isize - pos;
            from.copy_to(from.offset(1), count as usize);
        }
        *from = value;
        inner.len += 1;
    }

    // like
    // intset *intsetAdd(intset *is, int64_t value, uint8_t *success)
    fn insert(&mut self, value: i64) -> bool {
        let value_enc = Encoding::value_encoding(value);

        if value_enc > self.inner_ref().encoding {
            self.upgrade_and_add(value, value_enc);
            return true;
        }

        let (exist, pos) = self.search(value);
        if exist {
            return false;
        }

        let len = self.inner_ref().len;
        self.resize(len + 1);
        let len = len as isize;
        let need_move = pos < len;
        let inner = self.inner_ref();
        unsafe {
            match inner.encoding {
                INT16 => self.put_one(pos, value as i16, need_move),
                INT32 => self.put_one(pos, value as i32, need_move),
                INT64 => self.put_one(pos, value as i64, need_move),
            }
        }

        true
    }

    fn remove(&mut self, value: i64) -> bool {
        let inner = self.inner_ref();
        let enc = inner.encoding;
        let value_enc = Encoding::value_encoding(value);
        if value_enc > enc {
            return false;
        }

        let (exist, pos) = self.search(value);
        if !exist {
            return false;
        }
        let len = inner.len;
        let last_idx = (len - 1) as isize;
        if pos < last_idx {
            unsafe {
                let count = (last_idx - pos) as usize;
                match enc {
                    INT16 => {
                        let from = inner.contents.as_ptr() as *mut i16;
                        from.offset(1).copy_to(from, count);
                    }
                    INT32 => {
                        let from = inner.contents.as_ptr() as *mut i32;
                        from.offset(1).copy_to(from, count);
                    }
                    INT64 => {
                        let from = inner.contents.as_ptr() as *mut i64;
                        from.offset(1).copy_to(from, count);
                    }
                }
            }
        }

        self.resize(len - 1);
        self.inner_mut_ref().len -= 1;

        true
    }

    pub fn len(&self) -> u16 {
        self.inner_ref().len
    }

    // like
    // uint8_t intsetFind(intset *is, int64_t value)
    pub fn contain(&self, value: i64) -> bool {
        let value_enc = Encoding::value_encoding(value);
        if value_enc > self.inner_ref().encoding {
            return false;
        }

        let (exist, _) = self.search(value);
        exist
    }

    // like
    // int64_t intsetRandom(intset *is)
    pub fn random(&self) -> Option<i64> {
        self.get(0)
    }

    // like
    // uint8_t intsetGet(intset *is, uint32_t pos, int64_t *value)
    pub fn get(&self, index: isize) -> Option<i64> {
        if index < self.inner_ref().len as isize {
            unsafe { return Some(self.get_unchecked(index)); }
        }

        return None;
    }

    // like
    // static int64_t _intsetGet(intset *is, int pos)
    unsafe fn get_unchecked(&self, index: isize) -> i64 {
        let inner = self.inner_ref();
        Self::get_with_encoded(inner.contents.as_ptr(), index, inner.encoding)
    }

    // like
    // static int64_t _intsetGetEncoded(intset *is, int pos, uint8_t enc)
    unsafe fn get_with_encoded(contents: *const u8, index: isize, enc: Encoding) -> i64 {
        match enc {
            INT16 => {
                let contents = contents as *const i16;
                *contents.offset(index) as i64
            }
            INT32 => {
                let contents = contents as *const i32;
                *contents.offset(index) as i64
            }
            INT64 => {
                let contents = contents as *const i64;
                *contents.offset(index)
            }
        }
    }

    //fn move_

    // like
    // static void _intsetSet(intset *is, int pos, int64_t value)
    unsafe fn set_unchecked(&mut self, index: isize, value: i64) {
        let inner = self.inner_mut_ref();
        Self::set_with_encoded(inner.contents.as_mut_ptr(), index, value, inner.encoding);
    }

    unsafe fn set_with_encoded(contents: *mut u8, index: isize, value: i64, enc: Encoding) {
        match enc {
            INT16 => {
                let contents = contents as *mut i16;
                *contents.offset(index) = value as i16;
            }
            INT32 => {
                let contents = contents as *mut i32;
                *contents.offset(index) = value as i32;
            }
            INT64 => {
                let contents = contents as *mut i64;
                *contents.offset(index) = value;
            }
        }
    }
}

impl Drop for IntSet {
    fn drop(&mut self) {
        if !self.is_global_empty() {
            unsafe {
                z_free(self.0 as *const u8);
            }
        }
    }
}

impl Display for IntSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let len = self.len() as isize;
        let _ = write!(f, "{}", '[');
        unsafe {
            for i in 0..len {
                let _ = write!(f, "{}, ", self.get_unchecked(i));
            }
        }
        write!(f, "{}", ']')
    }
}

#[test]
fn test_basic() {
    assert_eq!(6, std::mem::size_of::<IntSetInner>());

    let mut set = IntSet::new();
    for i in 0..10 {
        set.insert(i);
        assert_eq!(Some(i), set.get(i as isize));
    }

    assert_eq!(Encoding::INT16, set.inner_ref().encoding);

    let v = i16::MIN as i64 - 1;
    set.insert(v);
    assert_eq!(Some(v), set.get(0));
    assert_eq!(Encoding::INT32, set.inner_ref().encoding);

    let v = i32::MAX as i64 + 1;
    set.insert(v);
    assert_eq!(Some(v), set.get((set.len() - 1) as isize));
    assert_eq!(Encoding::INT64, set.inner_ref().encoding);

    println!("{}", set);
}