//! Borrowed-byte parsers with C-style end pointers.

use crate::ByteView;

/// Parse a C double, setting errno on a range error and writing a C end pointer.
///
/// If `endptr` is non-null, store a pointer into the original byte view at the
/// first unconsumed byte. Failed conversion stores the view's start. If parsing
/// reaches a view end without a stored NUL, the pointer is one past the view
/// and must not be dereferenced. No terminating NUL is added.
///
/// # Safety
/// A non-null `endptr` must be aligned and valid for writing a `*mut u8`, with
/// exclusive write access during the call, and must not overlap the borrowed
/// input storage. Using the stored pointer requires that the input storage
/// remain live and that its original bounds and mutability be respected; the
/// pointer's mutable type does not grant permission to modify the input.
pub unsafe fn strtod_ptr<B: ByteView + ?Sized>(s: &B, endptr: *mut *mut u8) -> f64 {
    // SAFETY: The caller guarantees that endptr is null or valid to write.
    unsafe {
        parse_with_endptr(s, endptr, |bytes, endoff| {
            let (value, error) = crate::strtod_u_e(bytes, endoff);
            if let Some(error) = error {
                errno::set_errno(error);
            }
            value
        })
    }
}

/// The `strtof` counterpart of [`strtod_ptr`], with the same end-pointer behavior.
///
/// # Safety
/// A non-null `endptr` must be aligned and valid for writing a `*mut u8`, with
/// exclusive write access during the call, and must not overlap the borrowed
/// input storage. Using the stored pointer requires that the input storage
/// remain live and that its original bounds and mutability be respected.
pub unsafe fn strtof_ptr<B: ByteView + ?Sized>(s: &B, endptr: *mut *mut u8) -> f32 {
    // SAFETY: The caller guarantees that endptr is null or valid to write.
    unsafe {
        parse_with_endptr(s, endptr, |bytes, endoff| {
            let (value, error) = crate::strtof_u_e(bytes, endoff);
            if let Some(error) = error {
                errno::set_errno(error);
            }
            value
        })
    }
}

/// Parse a C long, setting errno on error and writing an end pointer as in
/// [`strtod_ptr`]. Base 0 detects octal and hexadecimal; other valid bases are
/// 2..=36. An invalid base returns zero, sets `EINVAL`, and stores the view's start.
///
/// # Safety
/// A non-null `endptr` must be aligned and valid for writing a `*mut u8`, with
/// exclusive write access during the call, and must not overlap the borrowed
/// input storage. Using the stored pointer requires that the input storage
/// remain live and that its original bounds and mutability be respected.
pub unsafe fn strtol_ptr<B: ByteView + ?Sized>(
    s: &B,
    endptr: *mut *mut u8,
    base: i32,
) -> libc::c_long {
    // SAFETY: The caller guarantees that endptr is null or valid to write.
    unsafe {
        parse_with_endptr(s, endptr, |bytes, endoff| {
            crate::strtol(bytes, endoff, base)
        })
    }
}

/// The unsigned-long counterpart of [`strtol_ptr`].
///
/// # Safety
/// A non-null `endptr` must be aligned and valid for writing a `*mut u8`, with
/// exclusive write access during the call, and must not overlap the borrowed
/// input storage. Using the stored pointer requires that the input storage
/// remain live and that its original bounds and mutability be respected.
pub unsafe fn strtoul_ptr<B: ByteView + ?Sized>(
    s: &B,
    endptr: *mut *mut u8,
    base: i32,
) -> libc::c_ulong {
    // SAFETY: The caller guarantees that endptr is null or valid to write.
    unsafe {
        parse_with_endptr(s, endptr, |bytes, endoff| {
            crate::strtoul(bytes, endoff, base)
        })
    }
}

/// The long-long counterpart of [`strtol_ptr`].
///
/// # Safety
/// A non-null `endptr` must be aligned and valid for writing a `*mut u8`, with
/// exclusive write access during the call, and must not overlap the borrowed
/// input storage. Using the stored pointer requires that the input storage
/// remain live and that its original bounds and mutability be respected.
pub unsafe fn strtoll_ptr<B: ByteView + ?Sized>(
    s: &B,
    endptr: *mut *mut u8,
    base: i32,
) -> libc::c_longlong {
    // SAFETY: The caller guarantees that endptr is null or valid to write.
    unsafe {
        parse_with_endptr(s, endptr, |bytes, endoff| {
            crate::strtoll(bytes, endoff, base)
        })
    }
}

/// The unsigned-long-long counterpart of [`strtol_ptr`].
///
/// # Safety
/// A non-null `endptr` must be aligned and valid for writing a `*mut u8`, with
/// exclusive write access during the call, and must not overlap the borrowed
/// input storage. Using the stored pointer requires that the input storage
/// remain live and that its original bounds and mutability be respected.
pub unsafe fn strtoull_ptr<B: ByteView + ?Sized>(
    s: &B,
    endptr: *mut *mut u8,
    base: i32,
) -> libc::c_ulonglong {
    // SAFETY: The caller guarantees that endptr is null or valid to write.
    unsafe {
        parse_with_endptr(s, endptr, |bytes, endoff| {
            crate::strtoull(bytes, endoff, base)
        })
    }
}

// The parser must return an end offset within the view (including its end).
// A non-null endptr must satisfy the public pointer adapters' safety requirements.
unsafe fn parse_with_endptr<B: ByteView + ?Sized, T>(
    s: &B,
    endptr: *mut *mut u8,
    parse: impl FnOnce(&[u8], &mut usize) -> T,
) -> T {
    // Capture the view once, so the parser and end pointer refer to the same
    // storage even for a ByteView implementation that selects between buffers.
    let bytes = s.view_bytes();
    let mut endoff = 0;
    let value = parse(bytes, &mut endoff);
    if !endptr.is_null() {
        // SAFETY: Our parsers leave endoff in 0..=bytes.len(), and the caller
        // guarantees that endptr is valid to write and disjoint from bytes.
        unsafe { *endptr = bytes.as_ptr().add(endoff) as *mut u8 };
    }
    value
}
