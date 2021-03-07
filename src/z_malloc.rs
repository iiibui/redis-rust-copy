extern "C" {
    fn malloc(size: usize) -> *const u8;
    fn free(ptr: *const u8);
    fn realloc(ptr: *const u8, size: usize) -> *const u8;
}

#[cfg(target_os = "macos")]
extern "C" {
    fn malloc_size(ptr: *const u8) -> usize;
}

#[cfg(target_os = "macos")]
unsafe fn z_malloc_size(ptr: *const u8) -> usize {
    malloc_size(ptr)
}


#[cfg(target_os = "linux")]
extern "C" {
    fn malloc_usable_size(ptr: *const u8) -> usize;
}

#[cfg(target_os = "linux")]
unsafe fn z_malloc_size(ptr: *const u8) -> usize {
    malloc_usable_size(ptr)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn z_try_malloc_usable(size: usize) -> (*const u8, usize) {
    unsafe {
        let p = malloc(size);
        if p.is_null() {
            (p, 0)
        } else {
            (p, z_malloc_size(p))
        }
    }
}

#[inline]
pub fn z_malloc_usable(size: usize) -> (*const u8, usize) {
    z_try_malloc_usable(size)
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn test_z_malloc_size() {
    unsafe {
        let (p, len) = z_try_malloc_usable(9);
        let pp = p as *mut u8;
        *pp = 31;
        assert_eq!(len, z_malloc_size(p));
        free(p)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn z_try_malloc_usable(size: usize) -> (*const u8, usize) {
    unsafe {
        let p = malloc(size);
        if p.is_null() {
            (p, 0)
        } else {
            (p, size)
        }
    }
}

#[inline]
pub unsafe fn z_free(ptr: *const u8) {
    free(ptr);
}

#[inline]
pub unsafe fn z_realloc_usable(ptr: *const u8, size: usize) -> (*const u8, usize) {
    let ptr = realloc(ptr, size);
    if ptr.is_null() {
        (ptr, 0)
    } else {
        (ptr, size)
    }
}
