// SPDX-License-Identifier: GPL-2.0
use crate::{alloc::AllocError, str::CStr};
use core::alloc::LayoutError;
use core::fmt;
use core::num::NonZeroI32;
use core::num::TryFromIntError;
use core::str::Utf8Error;
#[rustfmt::skip]
pub mod code {
    macro_rules! declare_err {
        ($err:tt $(,)? $($doc:expr),+) => {
            $(
            #[doc = $doc]
            )*
            pub const $err: super::Error =
                match super::Error::try_from_errno(-(crate::bindings::$err as i32)) {
                    Some(err) => err,
                    None => panic!("Invalid errno in `declare_err!`"),
                };
        };
    }
    declare_err!(EPERM, "Operation not permitted.");
    declare_err!(ENOENT, "No such file or directory.");
    declare_err!(ESRCH, "No such process.");
    declare_err!(EINTR, "Interrupted system call.");
    declare_err!(EIO, "I/O error.");
    declare_err!(ENXIO, "No such device or address.");
    declare_err!(E2BIG, "Argument list too long.");
    declare_err!(ENOEXEC, "Exec format error.");
    declare_err!(EBADF, "Bad file number.");
    declare_err!(ECHILD, "No child processes.");
    declare_err!(EAGAIN, "Try again.");
    declare_err!(ENOMEM, "Out of memory.");
    declare_err!(EACCES, "Permission denied.");
    declare_err!(EFAULT, "Bad address.");
    declare_err!(ENOTBLK, "Block device required.");
    declare_err!(EBUSY, "Device or resource busy.");
    declare_err!(EEXIST, "File exists.");
    declare_err!(EXDEV, "Cross-device link.");
    declare_err!(ENODEV, "No such device.");
    declare_err!(ENOTDIR, "Not a directory.");
    declare_err!(EISDIR, "Is a directory.");
    declare_err!(EINVAL, "Invalid argument.");
    declare_err!(ENFILE, "File table overflow.");
    declare_err!(EMFILE, "Too many open files.");
    declare_err!(ENOTTY, "Not a typewriter.");
    declare_err!(ETXTBSY, "Text file busy.");
    declare_err!(EFBIG, "File too large.");
    declare_err!(ENOSPC, "No space left on device.");
    declare_err!(ESPIPE, "Illegal seek.");
    declare_err!(EROFS, "Read-only file system.");
    declare_err!(EMLINK, "Too many links.");
    declare_err!(EPIPE, "Broken pipe.");
    declare_err!(EDOM, "Math argument out of domain of func.");
    declare_err!(ERANGE, "Math result not representable.");
    declare_err!(ERESTARTSYS, "Restart the system call.");
    declare_err!(ERESTARTNOINTR, "System call was interrupted by a signal and will be restarted.");
    declare_err!(ERESTARTNOHAND, "Restart if no handler.");
    declare_err!(ENOIOCTLCMD, "No ioctl command.");
    declare_err!(ERESTART_RESTARTBLOCK, "Restart by calling sys_restart_syscall.");
    declare_err!(EPROBE_DEFER, "Driver requests probe retry.");
    declare_err!(EOPENSTALE, "Open found a stale dentry.");
    declare_err!(ENOPARAM, "Parameter not supported.");
    declare_err!(EBADHANDLE, "Illegal NFS file handle.");
    declare_err!(ENOTSYNC, "Update synchronization mismatch.");
    declare_err!(EBADCOOKIE, "Cookie is stale.");
    declare_err!(ENOTSUPP, "Operation is not supported.");
    declare_err!(ETOOSMALL, "Buffer or request is too small.");
    declare_err!(ESERVERFAULT, "An untranslatable error occurred.");
    declare_err!(EBADTYPE, "Type not supported by server.");
    declare_err!(EJUKEBOX, "Request initiated, but will not complete before timeout.");
    declare_err!(EIOCBQUEUED, "iocb queued, will get completion event.");
    declare_err!(ERECALLCONFLICT, "Conflict with recalled state.");
    declare_err!(ENOGRACE, "NFS file lock reclaim refused.");
}
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Error(NonZeroI32);
unsafe impl crate::init::Zeroable for Option<Error> {}
impl Error {
    pub fn from_errno(errno: crate::ffi::c_int) -> Error {
        if errno < -(bindings::MAX_ERRNO as i32) || errno >= 0 {
            crate::pr_warn!(
                "attempted to create `Error` with out of range `errno`: {}\n",
                errno
            );
            return code::EINVAL;
        }
        unsafe { Error::from_errno_unchecked(errno) }
    }
    const fn try_from_errno(errno: crate::ffi::c_int) -> Option<Error> {
        if errno < -(bindings::MAX_ERRNO as i32) || errno >= 0 {
            return None;
        }
        Some(unsafe { Error::from_errno_unchecked(errno) })
    }
    const unsafe fn from_errno_unchecked(errno: crate::ffi::c_int) -> Error {
        Error(unsafe { NonZeroI32::new_unchecked(errno) })
    }
    pub fn to_errno(self) -> crate::ffi::c_int {
        self.0.get()
    }
    #[cfg(not(all()))]
    pub(crate) fn to_blk_status(self) -> bindings::blk_status_t {
        unsafe { bindings::errno_to_blk_status(self.0.get()) }
    }
    pub fn to_ptr<T>(self) -> *mut T {
        unsafe { bindings::ERR_PTR(self.0.get() as _) as *mut _ }
    }
    #[cfg(not(any(test, testlib)))]
    pub fn name(&self) -> Option<&'static CStr> {
        let ptr = unsafe { bindings::errname(-self.0.get()) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_char_ptr(ptr) })
        }
    }
    #[cfg(any(test, testlib))]
    pub fn name(&self) -> Option<&'static CStr> {
        None
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            None => f.debug_tuple("Error").field(&-self.0).finish(),
            Some(name) => f
                .debug_tuple(
                    unsafe { core::str::from_utf8_unchecked(name) },
                )
                .finish(),
        }
    }
}
impl From<AllocError> for Error {
    fn from(_: AllocError) -> Error {
        code::ENOMEM
    }
}
impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Error {
        code::EINVAL
    }
}
impl From<Utf8Error> for Error {
    fn from(_: Utf8Error) -> Error {
        code::EINVAL
    }
}
impl From<LayoutError> for Error {
    fn from(_: LayoutError) -> Error {
        code::ENOMEM
    }
}
impl From<core::fmt::Error> for Error {
    fn from(_: core::fmt::Error) -> Error {
        code::EINVAL
    }
}
impl From<core::convert::Infallible> for Error {
    fn from(e: core::convert::Infallible) -> Error {
        match e {}
    }
}
pub type Result<T = (), E = Error> = core::result::Result<T, E>;
pub fn to_result(err: crate::ffi::c_int) -> Result {
    if err < 0 {
        Err(Error::from_errno(err))
    } else {
        Ok(())
    }
}
pub fn from_err_ptr<T>(ptr: *mut T) -> Result<*mut T> {
    let const_ptr: *const crate::ffi::c_void = ptr.cast();
    if unsafe { bindings::IS_ERR(const_ptr) } {
        let err = unsafe { bindings::PTR_ERR(const_ptr) };
        #[allow(clippy::unnecessary_cast)]
        return Err(unsafe { Error::from_errno_unchecked(err as crate::ffi::c_int) });
    }
    Ok(ptr)
}
pub fn from_result<T, F>(f: F) -> T
where
    T: From<i16>,
    F: FnOnce() -> Result<T>,
{
    match f() {
        Ok(v) => v,
        Err(e) => T::from(e.to_errno() as i16),
    }
}
pub const VTABLE_DEFAULT_ERROR: &str =
    "This function must not be called, see the #[vtable] documentation.";
