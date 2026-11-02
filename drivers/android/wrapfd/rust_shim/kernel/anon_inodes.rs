// SPDX-License-Identifier: GPL-2.0
use crate::{
    bindings, build_error,
    error::{from_err_ptr, VTABLE_DEFAULT_ERROR},
    fs::{file::Offset, File},
    mm::virt::VmaNew,
    prelude::*,
    seq_file::SeqFile,
    types::{ARef, ForeignOwnable},
};
use core::{marker::PhantomData, mem::MaybeUninit};
#[vtable]
pub trait AnonInodeFileInterface: Sized {
    type Ptr: ForeignOwnable + Send + Sync;
    fn release(device: Self::Ptr, _file: &File) {
        drop(device);
    }
    fn llseek(
        _device: <Self::Ptr as ForeignOwnable>::Borrowed<'_>,
        _offs: Offset,
        _whence: ffi::c_int,
    ) -> Result<Offset> {
        build_error!(VTABLE_DEFAULT_ERROR)
    }
    fn mmap(
        _device: <Self::Ptr as ForeignOwnable>::Borrowed<'_>,
        _file: &File,
        _vma: &VmaNew,
    ) -> Result {
        build_error!(VTABLE_DEFAULT_ERROR)
    }
    fn ioctl(
        _device: <Self::Ptr as ForeignOwnable>::Borrowed<'_>,
        _file: &File,
        _cmd: u32,
        _arg: usize,
    ) -> Result<isize> {
        build_error!(VTABLE_DEFAULT_ERROR)
    }
    #[cfg(CONFIG_COMPAT)]
    fn compat_ioctl(
        _device: <Self::Ptr as ForeignOwnable>::Borrowed<'_>,
        _file: &File,
        _cmd: u32,
        _arg: usize,
    ) -> Result<isize> {
        build_error!(VTABLE_DEFAULT_ERROR)
    }
    #[cfg(CONFIG_PROC_FS)]
    fn show_fdinfo(
        _device: <Self::Ptr as ForeignOwnable>::Borrowed<'_>,
        _m: &SeqFile,
        _file: &File,
    ) {
        build_error!(VTABLE_DEFAULT_ERROR)
    }
}
pub struct AnonInodeFileVTable<T: AnonInodeFileInterface> {
    fops: bindings::file_operations,
    _marker: PhantomData<T>,
}
unsafe impl<T: AnonInodeFileInterface> Sync for AnonInodeFileVTable<T> {}
impl<T: AnonInodeFileInterface> AnonInodeFileVTable<T> {
    pub const fn new(owner: &'static ThisModule) -> Self {
        Self {
            fops: bindings::file_operations {
                owner: owner.as_ptr(),
                release: Some(bindings::kcfi_wrap_release),
                llseek: if T::HAS_LLSEEK {
                    Some(bindings::kcfi_wrap_llseek)
                } else {
                    None
                },
                mmap: if T::HAS_MMAP {
                    Some(bindings::kcfi_wrap_mmap)
                } else {
                    None
                },
                unlocked_ioctl: if T::HAS_IOCTL {
                    Some(bindings::kcfi_wrap_ioctl)
                } else {
                    None
                },
                #[cfg(CONFIG_COMPAT)]
                compat_ioctl: if T::HAS_COMPAT_IOCTL {
                    Some(bindings::kcfi_wrap_compat_ioctl)
                } else if T::HAS_IOCTL {
                    Some(bindings::compat_ptr_ioctl)
                } else {
                    None
                },
                #[cfg(CONFIG_PROC_FS)]
                show_fdinfo: if T::HAS_SHOW_FDINFO {
                    Some(bindings::kcfi_wrap_show_fdinfo)
                } else {
                    None
                },
                ..unsafe { MaybeUninit::zeroed().assume_init() }
            },
            _marker: PhantomData,
        }
    }
    pub fn borrow_private<'a>(
        &'static self,
        file: &'a File,
    ) -> Option<<T::Ptr as ForeignOwnable>::Borrowed<'a>> {
        let f_op = unsafe { (*file.as_ptr()).f_op };
        if !core::ptr::eq(f_op, &self.fops) {
            return None;
        }
        let private = unsafe { (*file.as_ptr()).private_data };
        Some(unsafe { <T::Ptr as ForeignOwnable>::borrow(private.cast()) })
    }
    pub unsafe extern "C" fn release(
        _inode: *mut bindings::inode,
        file: *mut bindings::file,
    ) -> ffi::c_int {
        let private = unsafe { (*file).private_data };
        let ptr = unsafe { <T::Ptr as ForeignOwnable>::from_foreign(private) };
        T::release(ptr, unsafe { File::from_raw_file(file) });
        0
    }
    pub unsafe extern "C" fn llseek(
        file: *mut bindings::file,
        offs: Offset,
        whence: ffi::c_int,
    ) -> Offset {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::Ptr as ForeignOwnable>::borrow(private.cast()) };
        match T::llseek(device, offs, whence) {
            Ok(pos) => pos,
            Err(err) => err.to_errno().into(),
        }
    }
    pub unsafe extern "C" fn mmap(
        file: *mut bindings::file,
        vma: *mut bindings::vm_area_struct,
    ) -> ffi::c_int {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::Ptr as ForeignOwnable>::borrow(private.cast()) };
        let area = unsafe { VmaNew::from_raw(vma) };
        let file = unsafe { File::from_raw_file(file) };
        match T::mmap(device, file, area) {
            Ok(()) => 0,
            Err(err) => err.to_errno(),
        }
    }
    pub unsafe extern "C" fn ioctl(
        file: *mut bindings::file,
        cmd: ffi::c_uint,
        arg: ffi::c_ulong,
    ) -> ffi::c_long {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::Ptr as ForeignOwnable>::borrow(private.cast()) };
        let file = unsafe { File::from_raw_file(file) };
        match T::ioctl(device, file, cmd, arg) {
            Ok(ret) => ret as ffi::c_long,
            Err(err) => err.to_errno() as ffi::c_long,
        }
    }
    #[cfg(CONFIG_COMPAT)]
    pub unsafe extern "C" fn compat_ioctl(
        file: *mut bindings::file,
        cmd: ffi::c_uint,
        arg: ffi::c_ulong,
    ) -> ffi::c_long {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::Ptr as ForeignOwnable>::borrow(private.cast()) };
        let file = unsafe { File::from_raw_file(file) };
        match T::compat_ioctl(device, file, cmd, arg) {
            Ok(ret) => ret as ffi::c_long,
            Err(err) => err.to_errno() as ffi::c_long,
        }
    }
    #[cfg(CONFIG_PROC_FS)]
    pub unsafe extern "C" fn show_fdinfo(
        seq_file: *mut bindings::seq_file,
        file: *mut bindings::file,
    ) {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::Ptr as ForeignOwnable>::borrow(private.cast()) };
        let file = unsafe { File::from_raw_file(file) };
        let m = unsafe { SeqFile::from_raw(seq_file) };
        T::show_fdinfo(device, m, file);
    }
}
pub fn anon_inode_create_getfile<T: AnonInodeFileInterface>(
    name: &CStr,
    ptr: T::Ptr,
    flags: u32,
    vtable: &'static AnonInodeFileVTable<T>,
) -> Result<ARef<File>, (Error, T::Ptr)> {
    let priv_data = ptr.into_foreign();
    let raw_file_or_err = from_err_ptr(unsafe {
        bindings::anon_inode_getfile_secure(
            name.as_ptr().cast(),
            &vtable.fops,
            priv_data as _,
            flags as i32,
            core::ptr::null(), // context_inode
        )
    });
    match raw_file_or_err {
        Err(err) => {
            let ptr = unsafe { <T::Ptr as ForeignOwnable>::from_foreign(priv_data) };
            Err((err, ptr))
        }
        Ok(raw_file) => {
            let file: &File = unsafe { File::from_raw_file(raw_file) };
            let file_ref = unsafe { ARef::from_raw(file.into()) };
            Ok(file_ref)
        }
    }
}
