// SPDX-License-Identifier: GPL-2.0
use crate::{
    bindings,
    error::{code::*, Error, Result},
    types::{ARef, AlwaysRefCounted, NotThreadSafe, Opaque},
};
use core::ptr;
pub type Offset = bindings::loff_t;
pub mod flags {
    pub const O_APPEND: u32 = bindings::O_APPEND;
    pub const O_ASYNC: u32 = bindings::FASYNC;
    pub const O_CLOEXEC: u32 = bindings::O_CLOEXEC;
    pub const O_CREAT: u32 = bindings::O_CREAT;
    pub const O_DIRECT: u32 = bindings::O_DIRECT;
    pub const O_DIRECTORY: u32 = bindings::O_DIRECTORY;
    pub const O_DSYNC: u32 = bindings::O_DSYNC;
    pub const O_EXCL: u32 = bindings::O_EXCL;
    pub const O_LARGEFILE: u32 = bindings::O_LARGEFILE;
    pub const O_NOATIME: u32 = bindings::O_NOATIME;
    pub const O_NOCTTY: u32 = bindings::O_NOCTTY;
    pub const O_NOFOLLOW: u32 = bindings::O_NOFOLLOW;
    pub const O_NONBLOCK: u32 = bindings::O_NONBLOCK;
    pub const O_NDELAY: u32 = bindings::O_NDELAY;
    pub const O_PATH: u32 = bindings::O_PATH;
    pub const O_SYNC: u32 = bindings::O_SYNC;
    pub const O_TMPFILE: u32 = bindings::O_TMPFILE;
    pub const O_TRUNC: u32 = bindings::O_TRUNC;
    pub const O_ACCMODE: u32 = bindings::O_ACCMODE;
    pub const O_RDONLY: u32 = bindings::O_RDONLY;
    pub const O_WRONLY: u32 = bindings::O_WRONLY;
    pub const O_RDWR: u32 = bindings::O_RDWR;
}
pub mod mode {
    pub const FMODE_READ: u32 = bindings::FMODE_READ;
    pub const FMODE_WRITE: u32 = bindings::FMODE_WRITE;
    pub const FMODE_CAN_READ: u32 = bindings::FMODE_CAN_READ;
    pub const FMODE_CAN_ODIRECT: u32 = bindings::FMODE_CAN_ODIRECT;
}
#[repr(transparent)]
pub struct File {
    inner: Opaque<bindings::file>,
}
unsafe impl Send for File {}
unsafe impl Sync for File {}
unsafe impl AlwaysRefCounted for File {
    #[inline]
    fn inc_ref(&self) {
        unsafe { bindings::get_file(self.as_ptr()) };
    }
    #[inline]
    unsafe fn dec_ref(obj: ptr::NonNull<File>) {
        unsafe { bindings::fput(obj.cast().as_ptr()) }
    }
}
#[repr(transparent)]
pub struct LocalFile {
    inner: Opaque<bindings::file>,
}
unsafe impl AlwaysRefCounted for LocalFile {
    #[inline]
    fn inc_ref(&self) {
        unsafe { bindings::get_file(self.as_ptr()) };
    }
    #[inline]
    unsafe fn dec_ref(obj: ptr::NonNull<LocalFile>) {
        unsafe { bindings::fput(obj.cast().as_ptr()) }
    }
}
impl LocalFile {
    #[inline]
    pub fn fget(fd: u32) -> Result<ARef<LocalFile>, BadFdError> {
        let ptr = ptr::NonNull::new(unsafe { bindings::fget(fd) }).ok_or(BadFdError)?;
        Ok(unsafe { ARef::from_raw(ptr.cast()) })
    }
    #[inline]
    pub unsafe fn from_raw_file<'a>(ptr: *const bindings::file) -> &'a LocalFile {
        unsafe { &*ptr.cast() }
    }
    #[inline]
    pub unsafe fn assume_no_fdget_pos(me: ARef<LocalFile>) -> ARef<File> {
        unsafe { ARef::from_raw(ARef::into_raw(me).cast()) }
    }
    #[inline]
    pub fn as_ptr(&self) -> *mut bindings::file {
        self.inner.get()
    }
    #[inline]
    pub fn flags(&self) -> u32 {
        unsafe { core::ptr::addr_of!((*self.as_ptr()).f_flags).read_volatile() }
    }
    #[inline]
    pub fn mode(&self) -> u32 {
        unsafe { (*self.as_ptr()).f_mode }
    }
    #[inline]
    pub fn i_size_read(&self) -> Offset {
        unsafe { bindings::i_size_read(bindings::file_inode(self.as_ptr())) }
    }
}
impl File {
    #[inline]
    pub unsafe fn from_raw_file<'a>(ptr: *const bindings::file) -> &'a File {
        unsafe { &*ptr.cast() }
    }
}
impl core::ops::Deref for File {
    type Target = LocalFile;
    #[inline]
    fn deref(&self) -> &LocalFile {
        unsafe { LocalFile::from_raw_file(self as *const File as *const bindings::file) }
    }
}
pub struct FileDescriptorReservation {
    fd: u32,
    _not_send: NotThreadSafe,
}
impl FileDescriptorReservation {
    #[inline]
    pub fn get_unused_fd_flags(flags: u32) -> Result<Self> {
        let fd: i32 = unsafe { bindings::get_unused_fd_flags(flags) };
        if fd < 0 {
            return Err(Error::from_errno(fd));
        }
        Ok(Self {
            fd: fd as u32,
            _not_send: NotThreadSafe,
        })
    }
    #[inline]
    pub fn reserved_fd(&self) -> u32 {
        self.fd
    }
    #[inline]
    pub fn fd_install(self, file: ARef<File>) {
        unsafe { bindings::fd_install(self.fd, file.as_ptr()) };
        core::mem::forget(self);
        core::mem::forget(file);
    }
}
impl Drop for FileDescriptorReservation {
    #[inline]
    fn drop(&mut self) {
        unsafe { bindings::put_unused_fd(self.fd) };
    }
}
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct BadFdError;
impl From<BadFdError> for Error {
    #[inline]
    fn from(_: BadFdError) -> Error {
        EBADF
    }
}
impl core::fmt::Debug for BadFdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.pad("EBADF")
    }
}
