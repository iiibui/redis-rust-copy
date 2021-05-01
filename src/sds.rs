use std::ops::{Sub, Deref, DerefMut};
use std::fmt::{Display, Formatter, Debug};
use std::fmt;
use std::cmp::Ordering;

use crate::z_malloc::{
    z_free as s_free,
    z_malloc_usable as s_malloc_usable,
    z_realloc_usable as s_realloc_usable,
    z_try_malloc_usable as s_try_malloc_usable,
};

const SDS_TYPE_8: u8 = 1;
const SDS_TYPE_16: u8 = 2;
const SDS_TYPE_32: u8 = 3;
const SDS_TYPE_64: u8 = 4;
const SDS_TYPE_MASK: u8 = 7;

const SDS_MAX_PRE_ALLOC: usize = 1024 * 1024;

#[repr(packed)]
struct SdsHdr<T> {
    len: T,
    alloc: T,
    _flags: u8,
    _buf: [u8; 0],
}

type SdsHdr8 = SdsHdr<u8>;
type SdsHdr16 = SdsHdr<u16>;
type SdsHdr32 = SdsHdr<u32>;
type SdsHdr64 = SdsHdr<u64>;

pub struct Sds(*const u8);

#[inline]
fn sds_hdr_size(sds_type: u8) -> usize {
    match sds_type & SDS_TYPE_MASK {
        SDS_TYPE_8 => std::mem::size_of::<SdsHdr8>(),
        SDS_TYPE_16 => std::mem::size_of::<SdsHdr16>(),
        SDS_TYPE_32 => std::mem::size_of::<SdsHdr32>(),
        SDS_TYPE_64 => std::mem::size_of::<SdsHdr64>(),
        _ => unimplemented!("sds_type unknown: {}", sds_type),
    }
}

#[inline]
fn sds_req_type(string_size: usize) -> u8 {
    if string_size < 1 << 8 {
        SDS_TYPE_8
    } else if string_size < 1 << 16 {
        SDS_TYPE_16
    } else if string_size < 1 << 32 {
        SDS_TYPE_32
    } else {
        SDS_TYPE_64
    }
}

#[inline]
fn sds_type_max_size(sds_type: u8) -> usize {
    match sds_type {
        SDS_TYPE_8 => u8::max_value() as usize,
        SDS_TYPE_16 => u16::max_value() as usize,
        SDS_TYPE_32 => u32::max_value() as usize,
        SDS_TYPE_64 => u64::max_value() as usize,
        _ => unimplemented!("sds_type unknown: {}", sds_type),
    }
}

impl<T: Sub<Output=T> + Into<u64> + Copy> SdsHdr<T> {
    // same as
    // #define SDS_HDR(T,s) ((struct sdshdr##T *)((s)-(sizeof(struct sdshdr##T))))
    #[inline]
    fn sds_hdr(sds: &Sds) -> &Self {
        unsafe {
            &*(sds.0.offset(-(std::mem::size_of::<Self>() as isize)) as *const Self)
        }
    }

    #[inline]
    fn mut_sds_hdr(sds: &Sds) -> &mut Self {
        unsafe {
            &mut *(sds.0.offset(-(std::mem::size_of::<Self>() as isize)) as *mut Self)
        }
    }

    #[inline]
    fn sds_len(&self) -> usize {
        self.len.into() as usize
    }

    // SDS_TYPE_64 in 32 bit mach would make unexpected fault
    #[inline]
    fn sds_alloc(&self) -> usize {
        self.alloc.into() as usize
    }

    // same as
    // inline size_t sdsavail(const sds s)
    #[inline]
    fn sds_avail(&self) -> usize {
        (self.alloc - self.len).into() as usize
    }
}

// global empty sds
static EMPTY_HDR: SdsHdr8 = SdsHdr8 {
    len: 0,
    alloc: 0,
    _flags: SDS_TYPE_8,
    _buf: [],
};

impl Sds {
    // same as
    // sds sdsnewlen(const void *init, size_t initlen)
    pub fn from_slice(init: &[u8]) -> Self {
        Self::from_raw_pointer(init.as_ptr(), init.len(), false)
    }

    // same as
    // sds sdstrynewlen(const void *init, size_t initlen)
    #[allow(dead_code)]
    fn try_from_slice(init: &[u8]) -> Self {
        Self::from_raw_pointer(init.as_ptr(), init.len(), true)
    }

    // like
    // sds sdsnew(const char *init)
    pub fn from_str(init: &str) -> Self {
        Self::from_raw_pointer(init.as_ptr(), init.len(), false)
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            let slice_ptr = std::ptr::slice_from_raw_parts(self.0, self.len());
            &*slice_ptr
        }
    }

    pub fn as_mut_slice(&self) -> &mut [u8] {
        unsafe {
            let slice_ptr = std::ptr::slice_from_raw_parts(self.0, self.len());
            &mut *(slice_ptr as *mut [u8])
        }
    }

    // may be illegal utf8 string
    pub fn as_str_uncheck(&self) -> &str {
        unsafe {
            let len = self.len();
            let s = String::from_raw_parts(self.0 as *mut u8, len, len);
            let fake = &*(&s as *const String);
            std::mem::forget(s);
            fake
        }
    }

    /// same as
    ///
    /// sds sdsempty(void)
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use redis_rust_copy::Sds;
    ///
    /// let s1 = Sds::empty();
    /// let s2 = Sds::empty();
    /// assert_eq!(s1, s2);
    /// ```
    #[inline]
    pub fn empty() -> Self {
        Sds(unsafe {
            (&EMPTY_HDR as *const SdsHdr8 as *const u8)
                .offset(std::mem::size_of_val(&EMPTY_HDR) as isize)
        })
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn is_global_empty(&self) -> bool {
        unsafe {
            self.0.offset(-(sds_hdr_size(self.type_code()) as isize))
                == (&EMPTY_HDR as *const SdsHdr8 as *const u8)
        }
    }

    // same as
    // sds _sdsnewlen(const void *init, size_t initlen, int trymalloc)
    // but no \0 at end any more
    fn from_raw_pointer(init: *const u8, init_len: usize, try_malloc: bool) -> Self {
        if init_len == 0 {
            return Self::empty();
        }
        let sds_type = sds_req_type(init_len);
        let hdr_size = sds_hdr_size(sds_type);
        let total_size = init_len + hdr_size;
        let (sh, mut usable) = if try_malloc {
            s_try_malloc_usable(total_size)
        } else {
            s_malloc_usable(total_size)
        };

        if sh.is_null() {
            panic!("malloc error");
        }

        usable -= hdr_size;
        usable = usable.min(sds_type_max_size(sds_type));

        let sds = Sds(unsafe { sh.offset(hdr_size as isize) });
        match sds_type {
            SDS_TYPE_8 => {
                let hdr = SdsHdr8::mut_sds_hdr(&sds);
                hdr.len = init_len as u8;
                hdr.alloc = usable as u8;
                hdr._flags = SDS_TYPE_8;
            }
            SDS_TYPE_16 => {
                let hdr = SdsHdr16::mut_sds_hdr(&sds);
                hdr.len = init_len as u16;
                hdr.alloc = usable as u16;
                hdr._flags = SDS_TYPE_16;
            }
            SDS_TYPE_32 => {
                let hdr = SdsHdr32::mut_sds_hdr(&sds);
                hdr.len = init_len as u32;
                hdr.alloc = usable as u32;
                hdr._flags = SDS_TYPE_32;
            }
            SDS_TYPE_64 => {
                let hdr = SdsHdr64::mut_sds_hdr(&sds);
                hdr.len = init_len as u64;
                hdr.alloc = usable as u64;
                hdr._flags = SDS_TYPE_64;
            }
            _ => unreachable!(),
        }

        if !init.is_null() {
            unsafe {
                init.copy_to_nonoverlapping(sds.0 as *mut u8, init_len);
            }
        }

        sds
    }

    // same as
    // void sdsclear(sds s)
    pub fn clear(&mut self) {
        unsafe { self.set_len_uncheck(0); }
    }

    // same as
    // sds sdsMakeRoomFor(sds s, size_t addlen)
    fn make_room_for(&mut self, inc_len: usize) {
        let avail = self.avail();
        if avail >= inc_len {
            return;
        }

        let len = self.len();
        let mut new_len = len + inc_len;
        if new_len < SDS_MAX_PRE_ALLOC {
            new_len *= 2;
        } else {
            new_len += SDS_MAX_PRE_ALLOC;
        }

        let old_type = self.type_code();
        let new_type = sds_req_type(new_len);
        let hdr_len = sds_hdr_size(new_type);
        let mut usable = unsafe {
            let sh = self.0.offset(-(sds_hdr_size(old_type) as isize));
            if old_type == new_type && !self.is_global_empty() {
                let (new_sh, usable) = s_realloc_usable(sh, hdr_len + new_len);
                if new_sh.is_null() {
                    panic!("s_realloc_usable {} size error", hdr_len + new_len);
                }
                self.0 = new_sh.offset(hdr_len as isize);
                usable
            } else {
                let (new_sh, usable) = s_malloc_usable(hdr_len + new_len);
                if new_sh.is_null() {
                    panic!("s_malloc_usable {} size error", hdr_len + new_len);
                }
                let new_s = new_sh.offset(hdr_len as isize) as *mut u8;
                self.0.copy_to_nonoverlapping(new_s, len);
                if !self.is_global_empty() {
                    s_free(sh);
                }

                self.0 = new_s;
                *(new_s.offset(-1) as *mut u8) = new_type;
                self.set_len_uncheck(len);
                usable
            }
        };

        usable -= hdr_len;
        usable = usable.min(sds_type_max_size(new_type));

        unsafe { self.set_alloc_uncheck(usable); }
    }

    // same as
    // sds sdscatlen(sds s, const void *t, size_t len)
    unsafe fn push_from_raw_pointer(&mut self, ptr: *const u8, len: usize) {
        if len == 0 {
            return;
        }
        let old_len = self.len();
        self.make_room_for(len);
        ptr.copy_to(self.0.offset(old_len as isize) as *mut u8, len);
        self.set_len_uncheck(old_len + len);
    }

    pub fn push_str(&mut self, s: &str) -> &mut Self {
        unsafe {
            self.push_from_raw_pointer(s.as_ptr(), s.len());
            self
        }
    }

    pub fn push_slice(&mut self, s: &[u8]) -> &mut Self {
        unsafe {
            self.push_from_raw_pointer(s.as_ptr(), s.len());
            self
        }
    }

    pub fn push_u8(&mut self, c: u8) -> &mut Self {
        unsafe { self.push_from_raw_pointer(&c, 1); }
        self
    }

    pub fn push(&mut self, s: &Sds) {
        unsafe {
            self.push_from_raw_pointer(s.0, s.len());
        }
    }

    #[inline]
    fn type_code(&self) -> u8 {
        unsafe {
            *self.0.offset(-1) as u8
        }
    }

    // same as
    // inline size_t sdslen(const sds s)
    #[inline]
    pub fn len(&self) -> usize {
        match self.type_code() {
            SDS_TYPE_8 => SdsHdr8::sds_hdr(self).sds_len(),
            SDS_TYPE_16 => SdsHdr16::sds_hdr(self).sds_len(),
            SDS_TYPE_32 => SdsHdr32::sds_hdr(self).sds_len(),
            SDS_TYPE_64 => SdsHdr64::sds_hdr(self).sds_len(),
            flags => unimplemented!("flags unknown: {}", flags),
        }
    }

    // same as
    // inline size_t sdsavail(const sds s)
    #[inline]
    pub fn alloc(&self) -> usize {
        match self.type_code() {
            SDS_TYPE_8 => SdsHdr8::sds_hdr(self).sds_alloc(),
            SDS_TYPE_16 => SdsHdr16::sds_hdr(self).sds_alloc(),
            SDS_TYPE_32 => SdsHdr32::sds_hdr(self).sds_alloc(),
            SDS_TYPE_64 => SdsHdr64::sds_hdr(self).sds_alloc(),
            flags => unimplemented!("flags unknown: {}", flags)
        }
    }

    // same as
    // inline size_t sdsavail(const sds s)
    #[inline]
    fn avail(&self) -> usize {
        match self.type_code() {
            SDS_TYPE_8 => SdsHdr8::sds_hdr(self).sds_avail(),
            SDS_TYPE_16 => SdsHdr16::sds_hdr(self).sds_avail(),
            SDS_TYPE_32 => SdsHdr32::sds_hdr(self).sds_avail(),
            SDS_TYPE_64 => SdsHdr64::sds_hdr(self).sds_avail(),
            flags => unimplemented!("flags unknown: {}", flags)
        }
    }

    // same as
    // inline void sdssetlen(sds s, size_t newlen)
    // but mark unsafe
    #[inline]
    unsafe fn set_len_uncheck(&mut self, new_len: usize) {
        match self.type_code() {
            SDS_TYPE_8 => SdsHdr8::mut_sds_hdr(self).len = new_len as u8,
            SDS_TYPE_16 => SdsHdr16::mut_sds_hdr(self).len = new_len as u16,
            SDS_TYPE_32 => SdsHdr32::mut_sds_hdr(self).len = new_len as u32,
            SDS_TYPE_64 => SdsHdr64::mut_sds_hdr(self).len = new_len as u64,
            flags => unimplemented!("flags unknown: {}", flags)
        };
    }

    // same as
    // inline void sdsinclen(sds s, size_t inc)
    // but mark unsafe
    #[inline]
    pub unsafe fn inc_len_uncheck(&mut self, inc: usize) {
        match self.type_code() {
            SDS_TYPE_8 => SdsHdr8::mut_sds_hdr(self).len += inc as u8,
            SDS_TYPE_16 => SdsHdr16::mut_sds_hdr(self).len += inc as u16,
            SDS_TYPE_32 => SdsHdr32::mut_sds_hdr(self).len += inc as u32,
            SDS_TYPE_64 => SdsHdr64::mut_sds_hdr(self).len += inc as u64,
            flags => unimplemented!("flags unknown: {}", flags)
        };
    }

    // same as
    // inline void sdssetalloc(sds s, size_t newlen)
    // but mark unsafe
    #[inline]
    unsafe fn set_alloc_uncheck(&mut self, alloc: usize) {
        match self.type_code() {
            SDS_TYPE_8 => SdsHdr8::mut_sds_hdr(self).alloc = alloc as u8,
            SDS_TYPE_16 => SdsHdr16::mut_sds_hdr(self).alloc = alloc as u16,
            SDS_TYPE_32 => SdsHdr32::mut_sds_hdr(self).alloc = alloc as u32,
            SDS_TYPE_64 => SdsHdr64::mut_sds_hdr(self).alloc = alloc as u64,
            flags => unimplemented!("flags unknown: {}", flags)
        };
    }
}

impl Display for Sds {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str_uncheck())
    }
}

impl Debug for Sds {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl PartialEq for Sds {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice().cmp(other.as_slice()) == Ordering::Equal
    }
}

impl Eq for Sds {}

impl PartialOrd for Sds {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl Ord for Sds {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl Deref for Sds {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for Sds {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl Clone for Sds {
    // same as
    // sds sdsdup(const sds s)
    fn clone(&self) -> Self {
        Self::from_raw_pointer(self.0, self.len(), false)
    }

    fn clone_from(&mut self, source: &Self) {
        let s = source.clone();
        {
            // trigger drop
            let _ = Sds(self.0);
        }
        self.0 = s.0;
    }
}

impl Drop for Sds {
    // same as
    // void sdsfree(sds s)
    fn drop(&mut self) {
        if !self.is_global_empty() {
            unsafe {
                s_free(self.0.offset(-(sds_hdr_size(self.type_code()) as isize)));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! test_sds_base {
        ($kind:ident, $flag:expr) => {
            let hdr = $kind {
            len: 0,
            alloc: 0,
            _flags: $flag,
            _buf: []
        };
        let p = (&hdr as *const $kind) as *const u8;
        unsafe {
            let mut sds = Sds(p.offset(std::mem::size_of_val(&hdr) as isize));
            assert_eq!(sds.len(), 0, "{} init len assert fail", stringify!($kind));
            assert_eq!(sds.alloc(), 0, "{} init alloc assert fail", stringify!($kind));
            assert_eq!(sds.avail(), 0, "{} init avail assert fail", stringify!($kind));

            sds.set_len_uncheck(1);
            assert_eq!(sds.len(), 1, "{} set_len_uncheck len assert fail", stringify!($kind));
            assert_eq!(sds.alloc(), 0, "{} set_len_uncheck alloc assert fail", stringify!($kind));
            // cannot call avail()

            sds.set_alloc_uncheck(2);
            assert_eq!(sds.len(), 1, "{} set_alloc_uncheck len assert fail", stringify!($kind));
            assert_eq!(sds.alloc(), 2, "{} set_alloc_uncheck alloc assert fail", stringify!($kind));
            assert_eq!(sds.avail(), 1, "{} set_alloc_uncheck avail assert fail", stringify!($kind));

            sds.inc_len_uncheck(1);
            assert_eq!(sds.len(), 2, "{} inc_len_uncheck len assert fail", stringify!($kind));
            assert_eq!(sds.alloc(), 2, "{} inc_len_uncheck alloc assert fail", stringify!($kind));
            assert_eq!(sds.avail(), 0, "{} inc_len_uncheck avail assert fail", stringify!($kind));
            std::mem::forget(sds);
        }
        };
    }

    #[test]
    fn test_all_sds_basic() {
        test_sds_base!(SdsHdr8, SDS_TYPE_8);
        test_sds_base!(SdsHdr16, SDS_TYPE_16);
        test_sds_base!(SdsHdr32, SDS_TYPE_32);
        test_sds_base!(SdsHdr64, SDS_TYPE_64);
    }

    #[test]
    fn test_sample() {
        let mut my_string = Sds::from_str("Hello World!");
        println!("{}", my_string);

        let buf = ['A' as u8, 'B' as u8, 'C' as u8];
        my_string = Sds::from_slice(&buf); // auto free before value
        println!("{} of len {}", my_string, my_string.len());

        my_string = Sds::empty(); // auto free before value
        println!("{}", my_string.len());

        my_string.push_str("Hello ").push_str("World!");
        println!("{}", my_string);

        let my_string2 = my_string.clone();
        println!("{} == {}", my_string, my_string2);

        my_string = Sds::from_str(" Hello World! ");
        let my_string_trim = my_string.as_str_uncheck().trim();
        println!("{}", my_string_trim);
        println!("{} {}", my_string.starts_with(&[' ' as u8]), my_string_trim.starts_with('H'));
    }
}
