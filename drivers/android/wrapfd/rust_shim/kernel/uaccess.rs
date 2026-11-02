// SPDX-License-Identifier: GPL-2.0
use crate::{
    alloc::{Allocator, Flags},
    bindings,
    error::Result,
    ffi::{c_char, c_void},
    prelude::*,
    types::{AsBytes, FromBytes},
};
use core::mem::{size_of, MaybeUninit};
pub type UserPtr = usize;
pub struct UserSlice {
    ptr: UserPtr,
    length: usize,
}
impl UserSlice {
    pub fn new(ptr: UserPtr, length: usize) -> Self {
        UserSlice { ptr, length }
    }
    pub fn read_all<A: Allocator>(self, buf: &mut Vec<u8, A>, flags: Flags) -> Result {
        self.reader().read_all(buf, flags)
    }
    pub fn reader(self) -> UserSliceReader {
        UserSliceReader {
            ptr: self.ptr,
            length: self.length,
        }
    }
    pub fn writer(self) -> UserSliceWriter {
        UserSliceWriter {
            ptr: self.ptr,
            length: self.length,
        }
    }
    pub fn reader_writer(self) -> (UserSliceReader, UserSliceWriter) {
        (
            UserSliceReader {
                ptr: self.ptr,
                length: self.length,
            },
            UserSliceWriter {
                ptr: self.ptr,
                length: self.length,
            },
        )
    }
}
pub struct UserSliceReader {
    ptr: UserPtr,
    length: usize,
}
impl UserSliceReader {
    pub fn skip(&mut self, num_skip: usize) -> Result {
        self.length = self.length.checked_sub(num_skip).ok_or(EFAULT)?;
        self.ptr = self.ptr.wrapping_add(num_skip);
        Ok(())
    }
    pub fn clone_reader(&self) -> UserSliceReader {
        UserSliceReader {
            ptr: self.ptr,
            length: self.length,
        }
    }
    pub fn len(&self) -> usize {
        self.length
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub fn read_raw(&mut self, out: &mut [MaybeUninit<u8>]) -> Result {
        let len = out.len();
        let out_ptr = out.as_mut_ptr().cast::<c_void>();
        if len > self.length {
            return Err(EFAULT);
        }
        let res = unsafe { bindings::copy_from_user(out_ptr, self.ptr as *const c_void, len) };
        if res != 0 {
            return Err(EFAULT);
        }
        self.ptr = self.ptr.wrapping_add(len);
        self.length -= len;
        Ok(())
    }
    pub fn read_slice(&mut self, out: &mut [u8]) -> Result {
        let out = unsafe { &mut *(out as *mut [u8] as *mut [MaybeUninit<u8>]) };
        self.read_raw(out)
    }
    pub fn read<T: FromBytes>(&mut self) -> Result<T> {
        let len = size_of::<T>();
        if len > self.length {
            return Err(EFAULT);
        }
        let mut out: MaybeUninit<T> = MaybeUninit::uninit();
        let res = unsafe {
            bindings::copy_from_user(
                out.as_mut_ptr().cast::<c_void>(),
                self.ptr as *const c_void,
                len as _,
            )
        };
        if res != 0 {
            return Err(EFAULT);
        }
        self.ptr = self.ptr.wrapping_add(len);
        self.length -= len;
        Ok(unsafe { out.assume_init() })
    }
    pub fn read_all<A: Allocator>(mut self, buf: &mut Vec<u8, A>, flags: Flags) -> Result {
        let len = self.length;
        buf.reserve(len, flags)?;
        self.read_raw(&mut buf.spare_capacity_mut()[..len])?;
        unsafe { buf.inc_len(len) };
        Ok(())
    }
    #[doc(alias = "strncpy_from_user")]
    pub fn strcpy_into_buf<'buf>(self, buf: &'buf mut [u8]) -> Result<&'buf CStr> {
        if buf.is_empty() {
            return Err(EINVAL);
        }
        let mut dst = unsafe { &mut *(core::ptr::from_mut(buf) as *mut [MaybeUninit<u8>]) };
        if dst.len() > self.length {
            dst = &mut dst[..self.length];
        }
        let mut len = raw_strncpy_from_user(dst, self.ptr)?;
        if len < dst.len() {
            len += 1;
        } else if len < buf.len() {
            return Err(EFAULT);
        } else {
            unsafe { *buf.last_mut().unwrap_unchecked() = 0 };
        }
        Ok(unsafe { CStr::from_bytes_with_nul_unchecked(&buf[..len]) })
    }
}
pub struct UserSliceWriter {
    ptr: UserPtr,
    length: usize,
}
impl UserSliceWriter {
    pub fn len(&self) -> usize {
        self.length
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub fn write_slice(&mut self, data: &[u8]) -> Result {
        let len = data.len();
        let data_ptr = data.as_ptr().cast::<c_void>();
        if len > self.length {
            return Err(EFAULT);
        }
        let res = unsafe { bindings::copy_to_user(self.ptr as *mut c_void, data_ptr, len) };
        if res != 0 {
            return Err(EFAULT);
        }
        self.ptr = self.ptr.wrapping_add(len);
        self.length -= len;
        Ok(())
    }
    pub fn write<T: AsBytes>(&mut self, value: &T) -> Result {
        let len = size_of::<T>();
        if len > self.length {
            return Err(EFAULT);
        }
        let res = unsafe {
            bindings::copy_to_user(
                self.ptr as *mut c_void,
                (value as *const T).cast::<c_void>(),
                len as _,
            )
        };
        if res != 0 {
            return Err(EFAULT);
        }
        self.ptr = self.ptr.wrapping_add(len);
        self.length -= len;
        Ok(())
    }
}
#[inline]
fn raw_strncpy_from_user(dst: &mut [MaybeUninit<u8>], src: UserPtr) -> Result<usize> {
    let len = dst.len() as isize;
    let res = unsafe {
        bindings::strncpy_from_user(dst.as_mut_ptr().cast::<c_char>(), src as *const c_char, len)
    };
    if res < 0 {
        return Err(Error::from_errno(res as i32));
    }
    #[cfg(CONFIG_RUST_OVERFLOW_CHECKS)]
    assert!(res <= len);
    Ok(res as usize)
}
