// SPDX-License-Identifier: GPL-2.0
use crate::alloc::{flags::*, AllocError, KVec};
use core::fmt::{self, Write};
use core::ops::{self, Deref, DerefMut, Index};
use crate::error::{code::*, Error};
pub mod parse_int;
#[repr(transparent)]
pub struct BStr([u8]);
impl BStr {
    #[inline]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub const fn from_bytes(bytes: &[u8]) -> &Self {
        unsafe { &*(bytes as *const [u8] as *const BStr) }
    }
}
impl fmt::Display for BStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &b in &self.0 {
            match b {
                b'\t' => f.write_str("\\t")?,
                b'\n' => f.write_str("\\n")?,
                b'\r' => f.write_str("\\r")?,
                0x20..=0x7e => f.write_char(b as char)?,
                _ => write!(f, "\\x{b:02x}")?,
            }
        }
        Ok(())
    }
}
impl fmt::Debug for BStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('"')?;
        for &b in &self.0 {
            match b {
                b'\t' => f.write_str("\\t")?,
                b'\n' => f.write_str("\\n")?,
                b'\r' => f.write_str("\\r")?,
                b'\"' => f.write_str("\\\"")?,
                b'\\' => f.write_str("\\\\")?,
                0x20..=0x7e => f.write_char(b as char)?,
                _ => write!(f, "\\x{b:02x}")?,
            }
        }
        f.write_char('"')
    }
}
impl Deref for BStr {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl AsRef<BStr> for [u8] {
    fn as_ref(&self) -> &BStr {
        BStr::from_bytes(self)
    }
}
impl AsRef<BStr> for BStr {
    fn as_ref(&self) -> &BStr {
        self
    }
}
#[macro_export]
macro_rules! b_str {
    ($str:literal) => {{
        const S: &'static str = $str;
        const C: &'static $crate::str::BStr = $crate::str::BStr::from_bytes(S.as_bytes());
        C
    }};
}
#[derive(Debug, Clone, Copy)]
pub enum CStrConvertError {
    InteriorNul,
    NotNulTerminated,
}
impl From<CStrConvertError> for Error {
    #[inline]
    fn from(_: CStrConvertError) -> Error {
        EINVAL
    }
}
#[repr(transparent)]
pub struct CStr([u8]);
impl CStr {
    #[inline]
    pub const fn len(&self) -> usize {
        self.len_with_nul() - 1
    }
    #[inline]
    pub const fn len_with_nul(&self) -> usize {
        if self.0.is_empty() {
            unsafe { core::hint::unreachable_unchecked() };
        }
        self.0.len()
    }
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub unsafe fn from_char_ptr<'a>(ptr: *const crate::ffi::c_char) -> &'a Self {
        let len = unsafe { bindings::strlen(ptr) } + 1;
        let bytes = unsafe { core::slice::from_raw_parts(ptr as _, len as _) };
        unsafe { Self::from_bytes_with_nul_unchecked(bytes) }
    }
    pub const fn from_bytes_with_nul(bytes: &[u8]) -> Result<&Self, CStrConvertError> {
        if bytes.is_empty() {
            return Err(CStrConvertError::NotNulTerminated);
        }
        if bytes[bytes.len() - 1] != 0 {
            return Err(CStrConvertError::NotNulTerminated);
        }
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] == 0 {
                return Err(CStrConvertError::InteriorNul);
            }
            i += 1;
        }
        Ok(unsafe { Self::from_bytes_with_nul_unchecked(bytes) })
    }
    #[inline]
    pub const unsafe fn from_bytes_with_nul_unchecked(bytes: &[u8]) -> &CStr {
        unsafe { core::mem::transmute(bytes) }
    }
    #[inline]
    pub unsafe fn from_bytes_with_nul_unchecked_mut(bytes: &mut [u8]) -> &mut CStr {
        unsafe { &mut *(bytes as *mut [u8] as *mut CStr) }
    }
    #[inline]
    pub const fn as_char_ptr(&self) -> *const crate::ffi::c_char {
        self.0.as_ptr() as _
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0[..self.len()]
    }
    #[inline]
    pub const fn as_bytes_with_nul(&self) -> &[u8] {
        &self.0
    }
    #[inline]
    pub fn to_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(self.as_bytes())
    }
    #[inline]
    pub unsafe fn as_str_unchecked(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(self.as_bytes()) }
    }
    pub fn to_cstring(&self) -> Result<CString, AllocError> {
        CString::try_from(self)
    }
    pub fn make_ascii_lowercase(&mut self) {
        self.0.make_ascii_lowercase();
    }
    pub fn make_ascii_uppercase(&mut self) {
        self.0.make_ascii_uppercase();
    }
    pub fn to_ascii_lowercase(&self) -> Result<CString, AllocError> {
        let mut s = self.to_cstring()?;
        s.make_ascii_lowercase();
        Ok(s)
    }
    pub fn to_ascii_uppercase(&self) -> Result<CString, AllocError> {
        let mut s = self.to_cstring()?;
        s.make_ascii_uppercase();
        Ok(s)
    }
}
impl fmt::Display for CStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &c in self.as_bytes() {
            if (0x20..0x7f).contains(&c) {
                f.write_char(c as char)?;
            } else {
                write!(f, "\\x{c:02x}")?;
            }
        }
        Ok(())
    }
}
impl fmt::Debug for CStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"")?;
        for &c in self.as_bytes() {
            match c {
                b'\"' => f.write_str("\\\"")?,
                0x20..=0x7e => f.write_char(c as char)?,
                _ => write!(f, "\\x{c:02x}")?,
            }
        }
        f.write_str("\"")
    }
}
impl AsRef<BStr> for CStr {
    #[inline]
    fn as_ref(&self) -> &BStr {
        BStr::from_bytes(self.as_bytes())
    }
}
impl Deref for CStr {
    type Target = BStr;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
impl Index<ops::RangeFrom<usize>> for CStr {
    type Output = CStr;
    #[inline]
    fn index(&self, index: ops::RangeFrom<usize>) -> &Self::Output {
        let _ = &self.as_bytes()[index.start..];
        unsafe { Self::from_bytes_with_nul_unchecked(&self.0[index.start..]) }
    }
}
impl Index<ops::RangeFull> for CStr {
    type Output = CStr;
    #[inline]
    fn index(&self, _index: ops::RangeFull) -> &Self::Output {
        self
    }
}
mod private {
    use core::ops;
    pub trait CStrIndex {}
    impl CStrIndex for usize {}
    impl CStrIndex for ops::Range<usize> {}
    impl CStrIndex for ops::RangeInclusive<usize> {}
    impl CStrIndex for ops::RangeToInclusive<usize> {}
}
impl<Idx> Index<Idx> for CStr
where
    Idx: private::CStrIndex,
    BStr: Index<Idx>,
{
    type Output = <BStr as Index<Idx>>::Output;
    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.as_ref()[index]
    }
}
#[macro_export]
macro_rules! c_str {
    ($str:expr) => {{
        const S: &str = concat!($str, "\0");
        const C: &$crate::str::CStr = match $crate::str::CStr::from_bytes_with_nul(S.as_bytes()) {
            Ok(v) => v,
            Err(_) => panic!("string contains interior NUL"),
        };
        C
    }};
}
#[cfg(test)]
mod tests {
    use super::*;
    struct String(CString);
    impl String {
        fn from_fmt(args: fmt::Arguments<'_>) -> Self {
            String(CString::try_from_fmt(args).unwrap())
        }
    }
    impl Deref for String {
        type Target = str;
        fn deref(&self) -> &str {
            self.0.to_str().unwrap()
        }
    }
    macro_rules! format {
        ($($f:tt)*) => ({
            &*String::from_fmt(kernel::fmt!($($f)*))
        })
    }
    const ALL_ASCII_CHARS: &'static str =
        "\\x01\\x02\\x03\\x04\\x05\\x06\\x07\\x08\\x09\\x0a\\x0b\\x0c\\x0d\\x0e\\x0f\
        \\x10\\x11\\x12\\x13\\x14\\x15\\x16\\x17\\x18\\x19\\x1a\\x1b\\x1c\\x1d\\x1e\\x1f \
        !\"#$%&'()*+,-./0123456789:;<=>?@\
        ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\\x7f\
        \\x80\\x81\\x82\\x83\\x84\\x85\\x86\\x87\\x88\\x89\\x8a\\x8b\\x8c\\x8d\\x8e\\x8f\
        \\x90\\x91\\x92\\x93\\x94\\x95\\x96\\x97\\x98\\x99\\x9a\\x9b\\x9c\\x9d\\x9e\\x9f\
        \\xa0\\xa1\\xa2\\xa3\\xa4\\xa5\\xa6\\xa7\\xa8\\xa9\\xaa\\xab\\xac\\xad\\xae\\xaf\
        \\xb0\\xb1\\xb2\\xb3\\xb4\\xb5\\xb6\\xb7\\xb8\\xb9\\xba\\xbb\\xbc\\xbd\\xbe\\xbf\
        \\xc0\\xc1\\xc2\\xc3\\xc4\\xc5\\xc6\\xc7\\xc8\\xc9\\xca\\xcb\\xcc\\xcd\\xce\\xcf\
        \\xd0\\xd1\\xd2\\xd3\\xd4\\xd5\\xd6\\xd7\\xd8\\xd9\\xda\\xdb\\xdc\\xdd\\xde\\xdf\
        \\xe0\\xe1\\xe2\\xe3\\xe4\\xe5\\xe6\\xe7\\xe8\\xe9\\xea\\xeb\\xec\\xed\\xee\\xef\
        \\xf0\\xf1\\xf2\\xf3\\xf4\\xf5\\xf6\\xf7\\xf8\\xf9\\xfa\\xfb\\xfc\\xfd\\xfe\\xff";
    #[test]
    fn test_cstr_to_str() {
        let good_bytes = b"\xf0\x9f\xa6\x80\0";
        let checked_cstr = CStr::from_bytes_with_nul(good_bytes).unwrap();
        let checked_str = checked_cstr.to_str().unwrap();
        assert_eq!(checked_str, "🦀");
    }
    #[test]
    #[should_panic]
    fn test_cstr_to_str_panic() {
        let bad_bytes = b"\xc3\x28\0";
        let checked_cstr = CStr::from_bytes_with_nul(bad_bytes).unwrap();
        checked_cstr.to_str().unwrap();
    }
    #[test]
    fn test_cstr_as_str_unchecked() {
        let good_bytes = b"\xf0\x9f\x90\xA7\0";
        let checked_cstr = CStr::from_bytes_with_nul(good_bytes).unwrap();
        let unchecked_str = unsafe { checked_cstr.as_str_unchecked() };
        assert_eq!(unchecked_str, "🐧");
    }
    #[test]
    fn test_cstr_display() {
        let hello_world = CStr::from_bytes_with_nul(b"hello, world!\0").unwrap();
        assert_eq!(format!("{hello_world}"), "hello, world!");
        let non_printables = CStr::from_bytes_with_nul(b"\x01\x09\x0a\0").unwrap();
        assert_eq!(format!("{non_printables}"), "\\x01\\x09\\x0a");
        let non_ascii = CStr::from_bytes_with_nul(b"d\xe9j\xe0 vu\0").unwrap();
        assert_eq!(format!("{non_ascii}"), "d\\xe9j\\xe0 vu");
        let good_bytes = CStr::from_bytes_with_nul(b"\xf0\x9f\xa6\x80\0").unwrap();
        assert_eq!(format!("{good_bytes}"), "\\xf0\\x9f\\xa6\\x80");
    }
    #[test]
    fn test_cstr_display_all_bytes() {
        let mut bytes: [u8; 256] = [0; 256];
        for i in u8::MIN..=u8::MAX {
            bytes[i as usize] = i.wrapping_add(1);
        }
        let cstr = CStr::from_bytes_with_nul(&bytes).unwrap();
        assert_eq!(format!("{cstr}"), ALL_ASCII_CHARS);
    }
    #[test]
    fn test_cstr_debug() {
        let hello_world = CStr::from_bytes_with_nul(b"hello, world!\0").unwrap();
        assert_eq!(format!("{hello_world:?}"), "\"hello, world!\"");
        let non_printables = CStr::from_bytes_with_nul(b"\x01\x09\x0a\0").unwrap();
        assert_eq!(format!("{non_printables:?}"), "\"\\x01\\x09\\x0a\"");
        let non_ascii = CStr::from_bytes_with_nul(b"d\xe9j\xe0 vu\0").unwrap();
        assert_eq!(format!("{non_ascii:?}"), "\"d\\xe9j\\xe0 vu\"");
        let good_bytes = CStr::from_bytes_with_nul(b"\xf0\x9f\xa6\x80\0").unwrap();
        assert_eq!(format!("{good_bytes:?}"), "\"\\xf0\\x9f\\xa6\\x80\"");
    }
    #[test]
    fn test_bstr_display() {
        let hello_world = BStr::from_bytes(b"hello, world!");
        assert_eq!(format!("{hello_world}"), "hello, world!");
        let escapes = BStr::from_bytes(b"_\t_\n_\r_\\_\'_\"_");
        assert_eq!(format!("{escapes}"), "_\\t_\\n_\\r_\\_'_\"_");
        let others = BStr::from_bytes(b"\x01");
        assert_eq!(format!("{others}"), "\\x01");
        let non_ascii = BStr::from_bytes(b"d\xe9j\xe0 vu");
        assert_eq!(format!("{non_ascii}"), "d\\xe9j\\xe0 vu");
        let good_bytes = BStr::from_bytes(b"\xf0\x9f\xa6\x80");
        assert_eq!(format!("{good_bytes}"), "\\xf0\\x9f\\xa6\\x80");
    }
    #[test]
    fn test_bstr_debug() {
        let hello_world = BStr::from_bytes(b"hello, world!");
        assert_eq!(format!("{hello_world:?}"), "\"hello, world!\"");
        let escapes = BStr::from_bytes(b"_\t_\n_\r_\\_\'_\"_");
        assert_eq!(format!("{escapes:?}"), "\"_\\t_\\n_\\r_\\\\_'_\\\"_\"");
        let others = BStr::from_bytes(b"\x01");
        assert_eq!(format!("{others:?}"), "\"\\x01\"");
        let non_ascii = BStr::from_bytes(b"d\xe9j\xe0 vu");
        assert_eq!(format!("{non_ascii:?}"), "\"d\\xe9j\\xe0 vu\"");
        let good_bytes = BStr::from_bytes(b"\xf0\x9f\xa6\x80");
        assert_eq!(format!("{good_bytes:?}"), "\"\\xf0\\x9f\\xa6\\x80\"");
    }
}
pub(crate) struct RawFormatter {
    beg: usize,
    pos: usize,
    end: usize,
}
impl RawFormatter {
    fn new() -> Self {
        Self {
            beg: 0,
            pos: 0,
            end: 0,
        }
    }
    pub(crate) unsafe fn from_ptrs(pos: *mut u8, end: *mut u8) -> Self {
        Self {
            beg: pos as _,
            pos: pos as _,
            end: end as _,
        }
    }
    pub(crate) unsafe fn from_buffer(buf: *mut u8, len: usize) -> Self {
        let pos = buf as usize;
        Self {
            pos,
            beg: pos,
            end: pos.saturating_add(len),
        }
    }
    pub(crate) fn pos(&self) -> *mut u8 {
        self.pos as _
    }
    pub(crate) fn bytes_written(&self) -> usize {
        self.pos - self.beg
    }
}
impl fmt::Write for RawFormatter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let pos_new = self.pos.saturating_add(s.len());
        let len_to_copy = core::cmp::min(pos_new, self.end).saturating_sub(self.pos);
        if len_to_copy > 0 {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    s.as_bytes().as_ptr(),
                    self.pos as *mut u8,
                    len_to_copy,
                )
            };
        }
        self.pos = pos_new;
        Ok(())
    }
}
pub(crate) struct Formatter(RawFormatter);
impl Formatter {
    pub(crate) unsafe fn from_buffer(buf: *mut u8, len: usize) -> Self {
        Self(unsafe { RawFormatter::from_buffer(buf, len) })
    }
}
impl Deref for Formatter {
    type Target = RawFormatter;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl fmt::Write for Formatter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s)?;
        if self.0.pos > self.0.end {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}
pub struct CString {
    buf: KVec<u8>,
}
impl CString {
    pub fn try_from_fmt(args: fmt::Arguments<'_>) -> Result<Self, Error> {
        let mut f = RawFormatter::new();
        f.write_fmt(args)?;
        f.write_str("\0")?;
        let size = f.bytes_written();
        let mut buf = KVec::with_capacity(size, GFP_KERNEL)?;
        let mut f = unsafe { Formatter::from_buffer(buf.as_mut_ptr(), size) };
        f.write_fmt(args)?;
        f.write_str("\0")?;
        unsafe { buf.inc_len(f.bytes_written()) };
        let ptr = unsafe { bindings::memchr(buf.as_ptr().cast(), 0, (f.bytes_written() - 1) as _) };
        if !ptr.is_null() {
            return Err(EINVAL);
        }
        Ok(Self { buf })
    }
}
impl Deref for CString {
    type Target = CStr;
    fn deref(&self) -> &Self::Target {
        unsafe { CStr::from_bytes_with_nul_unchecked(self.buf.as_slice()) }
    }
}
impl DerefMut for CString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { CStr::from_bytes_with_nul_unchecked_mut(self.buf.as_mut_slice()) }
    }
}
impl<'a> TryFrom<&'a CStr> for CString {
    type Error = AllocError;
    fn try_from(cstr: &'a CStr) -> Result<CString, AllocError> {
        let mut buf = KVec::new();
        buf.extend_from_slice(cstr.as_bytes_with_nul(), GFP_KERNEL)?;
        Ok(CString { buf })
    }
}
impl fmt::Debug for CString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
#[macro_export]
macro_rules! fmt {
    ($($f:tt)*) => ( core::format_args!($($f)*) )
}
