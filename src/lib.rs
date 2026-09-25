//! Safe byte-slice replacements for glibc's floating-point and integer
//! conversion functions: `atof`, `strtod`, `strtof`, `atoi`, `atol`, `atoll`,
//! `strtol`, `strtoul`, `strtoll`, and `strtoull`.
//!
//! Call site:  `atof(&raw mut buf as *mut c_char)`  ==>  `atof(&buf)`
//! (and delete the `extern "C" { fn atof(..) }` declaration c2rust emitted).
//!
//! Assumptions (both are the C defaults, so they hold unless the program
//! changed them): LC_NUMERIC is the "C" locale, and the FP rounding mode is
//! round-to-nearest-even.
//!
//! `CStr::from_bytes_until_nul(..)?.to_str()?.parse::<f64>()` looks close but differs from `atof` in several ways:
//!   • Whitespace: atof skips leading C whitespace, including `\v`, which `u8::is_ascii_whitespace` leaves out.
//!   • Trailing text: atof parses the longest valid prefix, so "1.5abc" gives 1.5 and "1e" gives 1.0. `parse` rejects both.
//!   • Hex floats: `atof` accepts them (0x1.8p3). `parse` doesn't.
//!   • NaN payloads: glibc turns `nan(123)` into a NaN carrying payload bits.
//!   • Failed conversion: `atof` returns +0.0 even after a `-` ("-abc"), while "-0x" gives -0.0.
//! Decimal input still goes through Rust's parse once the valid prefix has been cut out,
//! because Rust's parser is correctly rounded, just like glibc's.
pub fn atof(buf: &[u8]) -> f64 {
    // strtod stops at the NUL. With no NUL in the array the C call is UB;
    // stopping at the end of the array is a valid refinement of that.
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let mut endoff = 0;
    strtod_u_e(&buf[..end], &mut endoff).0
}

/// Parse without an end offset, setting thread-local errno on a range error.
pub fn strtod_n(s: &[u8]) -> f64 {
    let mut endoff: usize = 0;
    let (value, error) = strtod_u_e(s, &mut endoff);
    if let Some(error) = error {
        errno::set_errno(error);
    }
    value
}

/// Parse without an end offset, setting thread-local errno on a range error.
pub fn strtof_n(s: &[u8]) -> f32 {
    let mut endoff = 0;
    let (value, error) = strtof_u_e(s, &mut endoff);
    if let Some(error) = error {
        errno::set_errno(error);
    }
    value
}

/// Parse the longest valid `strtod` prefix and set `endoff` to its byte length.
/// If no conversion is possible, set `endoff` to zero. Return any range error
/// without changing the thread-local errno.
pub fn strtod_u_e(s: &[u8], endoff: &mut usize) -> (f64, Option<errno::Errno>) {
    strto_u_e::<f64>(s, endoff)
}

/// The `strtof` counterpart of `strtod_u_e`.
pub fn strtof_u_e(s: &[u8], endoff: &mut usize) -> (f32, Option<errno::Errno>) {
    strto_u_e::<f32>(s, endoff)
}

/// Parse a decimal C integer, discarding range errors like `atof`.
pub fn atoi(buf: &[u8]) -> libc::c_int {
    let mut endoff = 0;
    strtol_u_e(buf, &mut endoff, 10).0 as libc::c_int
}

/// Parse a decimal C long, discarding range errors like `atof`.
pub fn atol(buf: &[u8]) -> libc::c_long {
    let mut endoff = 0;
    strtol_u_e(buf, &mut endoff, 10).0
}

/// Parse a decimal C long long, discarding range errors like `atof`.
pub fn atoll(buf: &[u8]) -> libc::c_longlong {
    let mut endoff = 0;
    strtoll_u_e(buf, &mut endoff, 10).0
}

/// Parse a C long, recording the end offset and setting errno on error.
pub fn strtol(s: &[u8], endoff: &mut usize, base: i32) -> libc::c_long {
    let (value, error) = strtol_u_e(s, endoff, base);
    set_integer_errno(error);
    value
}

/// Parse a C unsigned long, recording the end offset and setting errno on error.
pub fn strtoul(s: &[u8], endoff: &mut usize, base: i32) -> libc::c_ulong {
    let (value, error) = strtoul_u_e(s, endoff, base);
    set_integer_errno(error);
    value
}

/// Parse a C long long, recording the end offset and setting errno on error.
pub fn strtoll(s: &[u8], endoff: &mut usize, base: i32) -> libc::c_longlong {
    let (value, error) = strtoll_u_e(s, endoff, base);
    set_integer_errno(error);
    value
}

/// Parse a C unsigned long long, recording the end offset and setting errno on error.
pub fn strtoull(s: &[u8], endoff: &mut usize, base: i32) -> libc::c_ulonglong {
    let (value, error) = strtoull_u_e(s, endoff, base);
    set_integer_errno(error);
    value
}

/// Parse a C long without an end offset, setting errno on error.
pub fn strtol_n(s: &[u8], base: i32) -> libc::c_long {
    let mut endoff = 0;
    strtol(s, &mut endoff, base)
}

/// Parse a C unsigned long without an end offset, setting errno on error.
pub fn strtoul_n(s: &[u8], base: i32) -> libc::c_ulong {
    let mut endoff = 0;
    strtoul(s, &mut endoff, base)
}

/// Parse a C long long without an end offset, setting errno on error.
pub fn strtoll_n(s: &[u8], base: i32) -> libc::c_longlong {
    let mut endoff = 0;
    strtoll(s, &mut endoff, base)
}

/// Parse a C unsigned long long without an end offset, setting errno on error.
pub fn strtoull_n(s: &[u8], base: i32) -> libc::c_ulonglong {
    let mut endoff = 0;
    strtoull(s, &mut endoff, base)
}

/// Parse a C long and return any error without changing thread-local errno.
/// Base 0 detects octal and hexadecimal prefixes; other valid bases are 2..=36.
/// An invalid base returns zero and `EINVAL`, with `endoff` set to zero.
pub fn strtol_u_e(s: &[u8], endoff: &mut usize, base: i32) -> (libc::c_long, Option<errno::Errno>) {
    let (bits, error) = parse_integer(s, endoff, base, libc::c_long::BITS, true);
    (bits as libc::c_long, error)
}

/// Parse a C unsigned long and return any error without changing thread-local errno.
pub fn strtoul_u_e(
    s: &[u8],
    endoff: &mut usize,
    base: i32,
) -> (libc::c_ulong, Option<errno::Errno>) {
    let (bits, error) = parse_integer(s, endoff, base, libc::c_ulong::BITS, false);
    (bits as libc::c_ulong, error)
}

/// Parse a C long long and return any error without changing thread-local errno.
pub fn strtoll_u_e(
    s: &[u8],
    endoff: &mut usize,
    base: i32,
) -> (libc::c_longlong, Option<errno::Errno>) {
    let (bits, error) = parse_integer(s, endoff, base, libc::c_longlong::BITS, true);
    (bits as libc::c_longlong, error)
}

/// Parse a C unsigned long long and return any error without changing thread-local errno.
pub fn strtoull_u_e(
    s: &[u8],
    endoff: &mut usize,
    base: i32,
) -> (libc::c_ulonglong, Option<errno::Errno>) {
    let (bits, error) = parse_integer(s, endoff, base, libc::c_ulonglong::BITS, false);
    (bits as libc::c_ulonglong, error)
}

fn set_integer_errno(error: Option<errno::Errno>) {
    if let Some(error) = error {
        errno::set_errno(error);
    }
}

fn ascii_digit(c: u8) -> Option<u32> {
    match c {
        b'0'..=b'9' => Some((c - b'0') as u32),
        b'a'..=b'z' => Some((c - b'a' + 10) as u32),
        b'A'..=b'Z' => Some((c - b'A' + 10) as u32),
        _ => None,
    }
}

/// Return the two's-complement bit pattern for a signed or unsigned C integer.
fn parse_integer(
    s: &[u8],
    endoff: &mut usize,
    base: i32,
    bits: u32,
    signed: bool,
) -> (u128, Option<errno::Errno>) {
    *endoff = 0;
    if base != 0 && !(2..=36).contains(&base) {
        return (0, Some(errno::Errno(libc::EINVAL)));
    }
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut i = 0;
    while matches!(at(i), b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r') {
        i += 1;
    }
    let negative = at(i) == b'-';
    if matches!(at(i), b'+' | b'-') {
        i += 1;
    }

    // The ordinary glibc strtol entry point uses the traditional grammar:
    // C23's 0b prefix requires its separate __isoc23_strtol entry point.
    let radix = if base == 0 {
        if at(i) == b'0' { 8 } else { 10 }
    } else {
        base as u32
    };
    let hex_prefix = (base == 0 || base == 16)
        && at(i) == b'0'
        && at(i + 1).eq_ignore_ascii_case(&b'x')
        && ascii_digit(at(i + 2)).is_some_and(|d| d < 16);
    let radix = if hex_prefix { 16 } else { radix };
    if hex_prefix {
        i += 2;
    }

    let mask = (1u128 << bits) - 1;
    let limit = if signed {
        (1u128 << (bits - 1)) - u128::from(!negative)
    } else {
        mask
    };
    let (mut magnitude, mut overflow, mut any) = (0u128, false, false);
    while let Some(digit) = ascii_digit(at(i)).filter(|&d| d < radix) {
        any = true;
        let digit = digit as u128;
        let radix = radix as u128;
        if !overflow {
            if magnitude > limit / radix || (magnitude == limit / radix && digit > limit % radix) {
                overflow = true;
            } else {
                magnitude = magnitude * radix + digit;
            }
        }
        i += 1;
    }
    if !any {
        return (0, None);
    }
    *endoff = i;
    if overflow {
        let value = if signed {
            if negative { 1u128 << (bits - 1) } else { limit }
        } else {
            mask
        };
        return (value, Some(errno::Errno(libc::ERANGE)));
    }
    let value = if negative {
        (0u128.wrapping_sub(magnitude)) & mask
    } else {
        magnitude
    };
    (value, None)
}

trait BinaryFloat: Copy + core::ops::Neg<Output = Self> {
    const SIGNIFICAND_BITS: i64;
    const FRACTION_BITS: u32;
    const MIN_NORMAL_EXP: i64;
    const MAX_EXP: i64;
    const SUBNORMAL_EXP: i64;
    const EXP_BIAS_MINUS_ONE: i64;
    const QNAN_BITS: u64;

    fn zero() -> Self;
    fn infinity() -> Self;
    fn from_bits(bits: u64) -> Self;
    fn to_bits(self) -> u64;
    fn parse_decimal(tok: &str) -> Self;
    fn is_infinite(self) -> bool;
    fn is_subnormal(self) -> bool;
    fn is_zero(self) -> bool;
}

impl BinaryFloat for f64 {
    const SIGNIFICAND_BITS: i64 = 53;
    const FRACTION_BITS: u32 = 52;
    const MIN_NORMAL_EXP: i64 = -1022;
    const MAX_EXP: i64 = 1023;
    const SUBNORMAL_EXP: i64 = -1074;
    const EXP_BIAS_MINUS_ONE: i64 = 1022;
    const QNAN_BITS: u64 = 0x7FF8_0000_0000_0000;

    fn zero() -> Self {
        0.0
    }
    fn infinity() -> Self {
        f64::INFINITY
    }
    fn from_bits(bits: u64) -> Self {
        f64::from_bits(bits)
    }
    fn to_bits(self) -> u64 {
        f64::to_bits(self)
    }
    fn parse_decimal(tok: &str) -> Self {
        tok.parse::<f64>().unwrap()
    }
    fn is_infinite(self) -> bool {
        f64::is_infinite(self)
    }
    fn is_subnormal(self) -> bool {
        f64::is_subnormal(self)
    }
    fn is_zero(self) -> bool {
        self == 0.0
    }
}

impl BinaryFloat for f32 {
    const SIGNIFICAND_BITS: i64 = 24;
    const FRACTION_BITS: u32 = 23;
    const MIN_NORMAL_EXP: i64 = -126;
    const MAX_EXP: i64 = 127;
    const SUBNORMAL_EXP: i64 = -149;
    const EXP_BIAS_MINUS_ONE: i64 = 126;
    const QNAN_BITS: u64 = 0x7FC0_0000;

    fn zero() -> Self {
        0.0
    }
    fn infinity() -> Self {
        f32::INFINITY
    }
    fn from_bits(bits: u64) -> Self {
        f32::from_bits(bits as u32)
    }
    fn to_bits(self) -> u64 {
        f32::to_bits(self) as u64
    }
    fn parse_decimal(tok: &str) -> Self {
        tok.parse::<f32>().unwrap()
    }
    fn is_infinite(self) -> bool {
        f32::is_infinite(self)
    }
    fn is_subnormal(self) -> bool {
        f32::is_subnormal(self)
    }
    fn is_zero(self) -> bool {
        self == 0.0
    }
}

fn strto_u_e<F: BinaryFloat>(s: &[u8], endoff: &mut usize) -> (F, Option<errno::Errno>) {
    *endoff = 0;
    let at = |i: usize| s.get(i).copied().unwrap_or(0);

    // C-locale isspace. Includes '\v' (0x0B), which u8::is_ascii_whitespace omits.
    let mut i = 0;
    while matches!(at(i), b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r') {
        i += 1;
    }
    let negative = at(i) == b'-';
    if matches!(at(i), b'+' | b'-') {
        i += 1;
    }
    let s = &s[i..];
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let sign = |x: F| if negative { -x } else { x };

    if !(at(0).is_ascii_digit() || (at(0) == b'.' && at(1).is_ascii_digit())) {
        if s.len() >= 3 && s[..3].eq_ignore_ascii_case(b"inf") {
            *endoff = i + if s.len() >= 8 && s[..8].eq_ignore_ascii_case(b"infinity") {
                8
            } else {
                3
            };
            return (sign(F::infinity()), None); // "infinity" has the same value
        }
        if s.len() >= 3 && s[..3].eq_ignore_ascii_case(b"nan") {
            let value = nan::<F>(&s[3..], endoff);
            *endoff += i + 3;
            return (sign(value), None);
        }
        return (F::zero(), None); // no conversion: glibc returns +0.0 even after a '-'
    }

    if at(0) == b'0' && at(1).eq_ignore_ascii_case(&b'x') {
        let (value, error) = hex::<F>(&s[2..], endoff);
        *endoff = if *endoff == 0 { i + 1 } else { i + 2 + *endoff };
        return (sign(value), error);
    }

    // Decimal: digits [. digits] [e [+-] digits], exponent only if it has digits.
    let digits = |mut k: usize| {
        while at(k).is_ascii_digit() {
            k += 1;
        }
        k
    };
    let mut n = digits(0);
    if at(n) == b'.' {
        n = digits(n + 1);
    }
    let mantissa_end = n;
    if matches!(at(n), b'e' | b'E') {
        let e = if matches!(at(n + 1), b'+' | b'-') {
            n + 2
        } else {
            n + 1
        };
        if at(e).is_ascii_digit() {
            n = digits(e);
        }
    }
    // The token is ASCII and matches Rust's float grammar, and Rust's parser is
    // correctly rounded, like glibc's. Neither unwrap can fail.
    let tok = core::str::from_utf8(&s[..n]).unwrap();
    *endoff = i + n;
    let value = F::parse_decimal(tok);
    let error = if value.is_infinite()
        || (value.is_zero() && s[..mantissa_end].iter().any(|c| matches!(c, b'1'..=b'9')))
        || (value.is_subnormal() && !decimal_is_exact::<F>(tok, value))
    {
        Some(errno::Errno(libc::ERANGE))
    } else {
        None
    };
    (sign(value), error)
}

/// Hex float body after "0x": correctly rounded, ties to even.
fn hex<F: BinaryFloat>(s: &[u8], endoff: &mut usize) -> (F, Option<errno::Errno>) {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let (mut m, mut exp, mut sticky) = (0u64, 0i64, false);
    let (mut k, mut any, mut point) = (0, false, false);
    loop {
        let c = at(k);
        if let Some(d) = (c as char).to_digit(16) {
            any = true;
            if m < 1 << 60 {
                m = m << 4 | d as u64;
                if point {
                    exp -= 4;
                }
            } else {
                sticky |= d != 0;
                if !point {
                    exp += 4;
                }
            }
        } else if c == b'.' && !point {
            point = true;
        } else {
            break;
        }
        k += 1;
    }
    if !any {
        return (F::zero(), None); // "0x" not followed by a hex mantissa: the value is the "0"
    }
    *endoff = k;
    if matches!(at(k), b'p' | b'P') {
        let neg = at(k + 1) == b'-';
        let mut e = if matches!(at(k + 1), b'+' | b'-') {
            k + 2
        } else {
            k + 1
        };
        if at(e).is_ascii_digit() {
            let mut p = 0i64;
            while at(e).is_ascii_digit() {
                p = p.saturating_mul(10).saturating_add((at(e) - b'0') as i64);
                e += 1;
            }
            *endoff = e;
            exp = exp.saturating_add(if neg { -p } else { p });
        }
    }
    if m == 0 {
        return (F::zero(), None);
    }

    let lz = m.leading_zeros() as i64;
    let m = m << lz; // value = m * 2^(e - 63), with bit 63 of m set
    let e = exp.saturating_sub(lz).saturating_add(63);
    if e > F::MAX_EXP {
        return (F::infinity(), Some(errno::Errno(libc::ERANGE)));
    }
    let keep = if e >= F::MIN_NORMAL_EXP {
        F::SIGNIFICAND_BITS
    } else {
        e.saturating_sub(F::SUBNORMAL_EXP).saturating_add(1)
    }; // bits that fit (subnormals: fewer)
    if keep < 0 {
        return (F::zero(), Some(errno::Errno(libc::ERANGE))); // below half the smallest subnormal
    }
    let drop = (64 - keep) as u32; // 11..=64
    let (q, rem) = if drop == 64 {
        (0, m)
    } else {
        (m >> drop, m & ((1 << drop) - 1))
    };
    let half = 1u64 << (drop - 1);
    let q = q + (rem > half || (rem == half && (sticky || q & 1 == 1))) as u64;
    let base = if e >= F::MIN_NORMAL_EXP {
        ((e + F::EXP_BIAS_MINUS_ONE) as u64) << F::FRACTION_BITS
    } else {
        0
    };
    let value = F::from_bits(base + q); // a carry out of the mantissa bumps the exponent (up to inf)
    let inexact = rem != 0 || sticky;
    let error = if value.is_infinite() || (value.is_subnormal() || value.is_zero()) && inexact {
        Some(errno::Errno(libc::ERANGE))
    } else {
        None
    };
    (value, error)
}

/// Compare a decimal token with the full decimal expansion of a subnormal.
/// Exact subnormals do not cause a range error.
fn decimal_is_exact<F: BinaryFloat>(tok: &str, value: F) -> bool {
    let (mantissa, exponent) = match tok.find(['e', 'E']) {
        Some(i) => {
            let exponent = &tok[i + 1..];
            let parsed = exponent.parse::<i128>().unwrap_or_else(|_| {
                if exponent.starts_with('-') {
                    i128::MIN
                } else {
                    i128::MAX
                }
            });
            (&tok[..i], parsed)
        }
        None => (tok, 0),
    };
    let fraction_len = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    let mut scale = exponent.saturating_sub(fraction_len as i128);
    let mut digits: Vec<u8> = mantissa.bytes().filter(|&c| c != b'.').collect();
    let first_nonzero = digits
        .iter()
        .position(|&c| c != b'0')
        .unwrap_or(digits.len());
    digits.drain(..first_nonzero);
    while digits.last() == Some(&b'0') {
        digits.pop();
        scale = scale.saturating_add(1);
    }

    // A subnormal is q * 2^SUBNORMAL_EXP, with an exact decimal expansion.
    let mut exact: Vec<u8> = (value.to_bits() & ((1u64 << F::FRACTION_BITS) - 1))
        .to_string()
        .bytes()
        .rev()
        .map(|c| c - b'0')
        .collect();
    for _ in 0..(-F::SUBNORMAL_EXP) {
        let mut carry = 0;
        for digit in &mut exact {
            let product = *digit * 5 + carry;
            *digit = product % 10;
            carry = product / 10;
        }
        if carry != 0 {
            exact.push(carry);
        }
    }
    let mut exact_scale = F::SUBNORMAL_EXP as i128;
    while exact.first() == Some(&0) {
        exact.remove(0);
        exact_scale += 1;
    }
    digits.iter().rev().map(|c| c - b'0').eq(exact) && scale == exact_scale
}

/// glibc NaN, including its "nan(n-char-sequence)" payload extension.
fn nan<F: BinaryFloat>(after: &[u8], endoff: &mut usize) -> F {
    let plain = F::from_bits(F::QNAN_BITS);
    let Some(body) = after.strip_prefix(b"(") else {
        return plain;
    };
    let n = body
        .iter()
        .position(|&c| !(c.is_ascii_alphanumeric() || c == b'_'))
        .unwrap_or(body.len());
    if body.get(n) != Some(&b')') {
        return plain;
    }
    *endoff = n + 2;
    // glibc sets the payload only if strtoull(seq, &end, 0) consumes all of seq.
    let payload_mask = (1u64 << (F::FRACTION_BITS - 1)) - 1;
    match strtoull_base0_whole(&body[..n]) {
        Some(v) if v & payload_mask != 0 => F::from_bits(F::QNAN_BITS | (v & payload_mask)),
        _ => plain,
    }
}

/// strtoull(seq, &end, 0) when `end` reaches the end of `seq`; saturates like strtoull.
fn strtoull_base0_whole(seq: &[u8]) -> Option<u64> {
    let (radix, digits) = match seq {
        [b'0', b'x' | b'X', rest @ ..] if !rest.is_empty() => (16, rest),
        [b'0', ..] => (8, seq),
        _ => (10, seq),
    };
    digits.iter().try_fold(0u64, |acc, &c| {
        let d = (c as char).to_digit(radix)? as u64;
        Some(
            acc.checked_mul(radix as u64)
                .and_then(|a| a.checked_add(d))
                .unwrap_or(u64::MAX),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        atof, atoi, atol, atoll, strtod_n, strtod_u_e, strtof_n, strtof_u_e, strtol, strtol_n,
        strtol_u_e, strtoll, strtoll_n, strtoll_u_e, strtoul, strtoul_n, strtoul_u_e, strtoull,
        strtoull_n, strtoull_u_e,
    };

    #[test]
    fn endoff_for_decimal_and_failed_conversions() {
        for (input, expected, end) in [
            ("", 0.0, 0),
            (" \t-", 0.0, 0),
            ("  +.x", 0.0, 0),
            ("  +12.5tail", 12.5, 7),
            ("  -0", -0.0, 4),
            ("1e+", 1.0, 1),
            ("1e-2rest", 0.01, 4),
            (".5.7", 0.5, 2),
        ] {
            let mut endoff = usize::MAX;
            let (value, _) = strtod_u_e(input.as_bytes(), &mut endoff);
            assert_eq!(endoff, end, "{input:?}");
            assert_eq!(value.to_bits(), f64::to_bits(expected), "{input:?}");
        }
    }

    #[test]
    fn endoff_for_hexadecimal_and_exponents() {
        for (input, expected, end) in [
            ("0x", 0.0, 1),
            (" -0x.", -0.0, 3),
            ("0x0p2!", 0.0, 5),
            ("0x1p+", 1.0, 3),
            ("  +0X1.8p-2tail", 0.375, 11),
            ("0x1.0z", 1.0, 5),
            ("0x1p1024!", f64::INFINITY, 8),
            ("0x1p-1075!", 0.0, 9),
        ] {
            let mut endoff = usize::MAX;
            let (value, _) = strtod_u_e(input.as_bytes(), &mut endoff);
            assert_eq!(endoff, end, "{input:?}");
            assert_eq!(value.to_bits(), f64::to_bits(expected), "{input:?}");
        }
    }

    #[test]
    fn endoff_for_infinity_and_nan() {
        for (input, end) in [
            ("INF!", 3),
            (" -InFiNiTy!", 10),
            ("infinite", 3),
            ("nan", 3),
            (" nan(123)tail", 9),
            ("nan(foo)tail", 8),
            ("nan()tail", 5),
            ("nan(abc", 3),
            ("nan(12-x)", 3),
        ] {
            let mut endoff = usize::MAX;
            let (value, _) = strtod_u_e(input.as_bytes(), &mut endoff);
            assert_eq!(endoff, end, "{input:?}");
            assert!(value.is_infinite() || value.is_nan(), "{input:?}");
        }
    }

    #[test]
    fn atof_stops_at_nul() {
        assert_eq!(atof(b"  2.5\0ignored"), 2.5);
        assert_eq!(atof(b"-0x1p2\0ignored"), -4.0);
    }

    #[test]
    fn range_errors_are_returned_without_setting_errno() {
        let old = errno::errno();
        errno::set_errno(errno::Errno(libc::EINVAL));
        for (input, expected_error) in [
            ("1e309", true),
            ("1e-324", true),
            ("5e-324", true),
            ("0x1p1024", true),
            ("0x1p-1075", true),
            ("0x1p-1074", false),
            ("0x1p-1023", false),
            ("0e-9999", false),
            ("2.2250738585072013e-308", false),
            ("inf", false),
        ] {
            let mut endoff = 0;
            let (_, error) = strtod_u_e(input.as_bytes(), &mut endoff);
            assert_eq!(
                error,
                expected_error.then_some(errno::Errno(libc::ERANGE)),
                "{input}"
            );
            assert_eq!(errno::errno(), errno::Errno(libc::EINVAL), "{input}");
        }
        let exact_subnormal = format!("{:.1074}", f64::from_bits(1));
        let mut endoff = 0;
        let (value, error) = strtod_u_e(exact_subnormal.as_bytes(), &mut endoff);
        assert_eq!(value.to_bits(), 1);
        assert_eq!(endoff, exact_subnormal.len());
        assert_eq!(error, None);
        errno::set_errno(old);
    }

    #[test]
    fn strtod_n_sets_errno_only_for_range_errors() {
        let old = errno::errno();
        errno::set_errno(errno::Errno(libc::EINVAL));
        assert_eq!(strtod_n(b"12.5"), 12.5);
        assert_eq!(errno::errno(), errno::Errno(libc::EINVAL));
        assert!(strtod_n(b"1e309").is_infinite());
        assert_eq!(errno::errno(), errno::Errno(libc::ERANGE));
        errno::set_errno(errno::Errno(libc::EINVAL));
        assert_eq!(strtod_n(b"-1e-9999").to_bits(), (-0.0f64).to_bits());
        assert_eq!(errno::errno(), errno::Errno(libc::ERANGE));
        errno::set_errno(errno::Errno(libc::EINVAL));
        assert!(atof(b"1e309\0").is_infinite());
        assert_eq!(errno::errno(), errno::Errno(libc::EINVAL));
        errno::set_errno(old);
    }

    #[test]
    fn strtof_prefixes_and_special_values() {
        for (input, expected_bits, end) in [
            (" \t-", 0.0f32.to_bits(), 0),
            ("  +12.5tail", 12.5f32.to_bits(), 7),
            ("-0", (-0.0f32).to_bits(), 2),
            ("1e+", 1.0f32.to_bits(), 1),
            ("  +0X1.8p-2tail", 0.375f32.to_bits(), 11),
            ("0x", 0.0f32.to_bits(), 1),
            ("-InFiNiTy!", f32::NEG_INFINITY.to_bits(), 9),
            ("nan(123)tail", 0x7fc0_007b, 8),
            ("-nan(0x3fffff)", 0xffff_ffff, 14),
        ] {
            let mut endoff = usize::MAX;
            let (value, error) = strtof_u_e(input.as_bytes(), &mut endoff);
            assert_eq!(endoff, end, "{input:?}");
            assert_eq!(value.to_bits(), expected_bits, "{input:?}");
            assert_eq!(error, None, "{input:?}");
        }
    }

    #[test]
    fn strtof_rounds_directly_to_f32() {
        for (input, bits) in [
            ("1.000000059604644775390625", 0x3f80_0000),
            ("1.0000000596046447753906250000001", 0x3f80_0001),
            ("0x1.000001p0", 0x3f80_0000),
            ("0x1.000001000001p0", 0x3f80_0001),
            ("0x1p-149", 1),
        ] {
            let mut endoff = 0;
            let (value, error) = strtof_u_e(input.as_bytes(), &mut endoff);
            assert_eq!(endoff, input.len(), "{input}");
            assert_eq!(value.to_bits(), bits, "{input}");
            assert_eq!(error, None, "{input}");
        }
    }

    #[test]
    fn strtof_range_errors_and_errno() {
        let old = errno::errno();
        errno::set_errno(errno::Errno(libc::EINVAL));
        for (input, bits, range_error) in [
            ("1e39", f32::INFINITY.to_bits(), true),
            ("1e-46", 0, true),
            ("1e-45", 1, true),
            ("0x1p128", f32::INFINITY.to_bits(), true),
            ("0x1.fffffep127", f32::MAX.to_bits(), false),
            ("0x1.ffffffp127", f32::INFINITY.to_bits(), true),
            ("0x1p-150", 0, true),
            ("0x1p-149", 1, false),
            ("0x1.1p-149", 1, true),
            ("0e-9999", 0, false),
            ("inf", f32::INFINITY.to_bits(), false),
        ] {
            let mut endoff = 0;
            let (value, error) = strtof_u_e(input.as_bytes(), &mut endoff);
            assert_eq!(endoff, input.len(), "{input}");
            assert_eq!(value.to_bits(), bits, "{input}");
            assert_eq!(
                error,
                range_error.then_some(errno::Errno(libc::ERANGE)),
                "{input}"
            );
            assert_eq!(errno::errno(), errno::Errno(libc::EINVAL), "{input}");
        }
        let exact_subnormal = format!("{:.149}", f32::from_bits(1));
        let mut endoff = 0;
        let (value, error) = strtof_u_e(exact_subnormal.as_bytes(), &mut endoff);
        assert_eq!(value.to_bits(), 1);
        assert_eq!(error, None);
        assert_eq!(endoff, exact_subnormal.len());

        assert!(strtof_n(b"1e39").is_infinite());
        assert_eq!(errno::errno(), errno::Errno(libc::ERANGE));
        errno::set_errno(errno::Errno(libc::EINVAL));
        assert_eq!(strtof_n(b"1.5"), 1.5);
        assert_eq!(errno::errno(), errno::Errno(libc::EINVAL));
        errno::set_errno(old);
    }

    #[test]
    fn integer_prefixes_and_bases() {
        for (input, base, expected, end) in [
            ("", 10, 0, 0),
            (" \t-", 10, 0, 0),
            ("\x0b-42\0ignored", 10, -42, 4),
            ("  +0x1f!", 0, 31, 7),
            ("077tail", 0, 63, 3),
            ("09", 0, 0, 1),
            ("0x", 0, 0, 1),
            ("0xg", 16, 0, 1),
            ("0b101", 0, 0, 1),
            ("0b101", 2, 0, 1),
            ("zZ", 36, 1295, 2),
            ("+12abc", 10, 12, 3),
        ] {
            let mut endoff = usize::MAX;
            let (value, error) = strtol_u_e(input.as_bytes(), &mut endoff, base);
            assert_eq!(value, expected, "{input:?}, base {base}");
            assert_eq!(endoff, end, "{input:?}, base {base}");
            assert_eq!(error, None, "{input:?}, base {base}");
        }
    }

    #[test]
    fn integer_limits_and_unsigned_negation() {
        let long_max = libc::c_long::MAX as u128;
        let long_min = libc::c_long::MIN as i128;
        for (input, expected, error) in [
            (long_max.to_string(), libc::c_long::MAX, None),
            (
                (long_max + 1).to_string(),
                libc::c_long::MAX,
                Some(errno::Errno(libc::ERANGE)),
            ),
            (long_min.to_string(), libc::c_long::MIN, None),
            (
                (long_min - 1).to_string(),
                libc::c_long::MIN,
                Some(errno::Errno(libc::ERANGE)),
            ),
        ] {
            let mut endoff = 0;
            let (value, actual_error) = strtol_u_e(input.as_bytes(), &mut endoff, 10);
            assert_eq!(
                (value, actual_error, endoff),
                (expected, error, input.len()),
                "{input}"
            );
        }

        let ulong_max = libc::c_ulong::MAX as u128;
        for (input, expected, error) in [
            ("-1".to_owned(), libc::c_ulong::MAX, None),
            (ulong_max.to_string(), libc::c_ulong::MAX, None),
            (
                (ulong_max + 1).to_string(),
                libc::c_ulong::MAX,
                Some(errno::Errno(libc::ERANGE)),
            ),
            (
                format!("-{}", ulong_max + 1),
                libc::c_ulong::MAX,
                Some(errno::Errno(libc::ERANGE)),
            ),
        ] {
            let mut endoff = 0;
            let (value, actual_error) = strtoul_u_e(input.as_bytes(), &mut endoff, 10);
            assert_eq!(
                (value, actual_error, endoff),
                (expected, error, input.len()),
                "{input}"
            );
        }

        let mut endoff = 0;
        assert_eq!(
            strtoll_u_e(
                libc::c_longlong::MIN.to_string().as_bytes(),
                &mut endoff,
                10
            )
            .0,
            libc::c_longlong::MIN
        );
        assert_eq!(
            strtoull_u_e(b"-1", &mut endoff, 10),
            (libc::c_ulonglong::MAX, None)
        );
    }

    #[test]
    fn integer_errno_and_wrappers() {
        let old = errno::errno();
        errno::set_errno(errno::Errno(libc::EACCES));
        let mut endoff = usize::MAX;
        assert_eq!(
            strtol_u_e(b"10", &mut endoff, 1),
            (0, Some(errno::Errno(libc::EINVAL)))
        );
        assert_eq!(endoff, 0);
        assert_eq!(errno::errno(), errno::Errno(libc::EACCES));

        assert_eq!(strtol(b"-12tail", &mut endoff, 10), -12);
        assert_eq!(endoff, 3);
        assert_eq!(errno::errno(), errno::Errno(libc::EACCES));
        assert_eq!(strtoul(b"-1", &mut endoff, 10), libc::c_ulong::MAX);
        assert_eq!(strtoll(b"42", &mut endoff, 10), 42);
        assert_eq!(strtoull(b"42", &mut endoff, 10), 42);
        assert_eq!(strtol_n(b"42", 10), 42);
        assert_eq!(strtoul_n(b"42", 10), 42);
        assert_eq!(strtoll_n(b"42", 10), 42);
        assert_eq!(strtoull_n(b"42", 10), 42);

        assert_eq!(strtol(b"10", &mut endoff, 37), 0);
        assert_eq!(errno::errno(), errno::Errno(libc::EINVAL));
        errno::set_errno(errno::Errno(libc::EACCES));
        assert_eq!(
            strtol_n(format!("{}0", libc::c_long::MAX).as_bytes(), 10),
            libc::c_long::MAX
        );
        assert_eq!(errno::errno(), errno::Errno(libc::ERANGE));

        errno::set_errno(errno::Errno(libc::EACCES));
        assert_eq!(atoi(b"  -123\0ignored"), -123);
        assert_eq!(atol(b"  -123\0ignored"), -123);
        assert_eq!(atoll(b"  -123\0ignored"), -123);
        assert_eq!(
            atol(format!("{}0", libc::c_long::MAX).as_bytes()),
            libc::c_long::MAX
        );
        assert_eq!(errno::errno(), errno::Errno(libc::EACCES));
        errno::set_errno(old);
    }
}
