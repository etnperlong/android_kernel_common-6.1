// SPDX-License-Identifier: GPL-2.0
use crate::{
    bindings, container_of,
    device::Device,
    error::{to_result, Error, Result, VTABLE_DEFAULT_ERROR},
    ffi::{c_int, c_long, c_uint, c_ulong, c_void},
    fs::file::Offset,
    fs::{File, LocalFile},
    mm::virt::VmaNew,
    prelude::*,
    seq_file::SeqFile,
    str::CStr,
    types::{AsBytes, ForeignOwnable, Opaque},
};
use core::{marker::PhantomData, mem::MaybeUninit, ops::Deref, pin::Pin, ptr::NonNull};
#[derive(Copy, Clone)]
pub struct MiscDeviceOptions {
    pub name: &'static CStr,
}
impl MiscDeviceOptions {
    pub const fn into_raw<T: MiscDevice>(self) -> bindings::miscdevice {
        let mut result: bindings::miscdevice = unsafe { MaybeUninit::zeroed().assume_init() };
        result.minor = bindings::MISC_DYNAMIC_MINOR as _;
        result.name = self.name.as_char_ptr();
        result.fops = MiscdeviceVTable::<T>::build();
        result
    }
}
#[repr(C)]
#[pin_data(PinnedDrop)]
pub struct MiscDeviceRegistration<T> {
    #[pin]
    inner: Opaque<bindings::miscdevice>,
    #[pin]
    data: T,
}
unsafe impl<T: Send> Send for MiscDeviceRegistration<T> {}
unsafe impl<T: Sync> Sync for MiscDeviceRegistration<T> {}
impl<T: MiscDevice> MiscDeviceRegistration<T> {
    pub fn register(
        opts: MiscDeviceOptions,
        data: impl PinInit<T, Error>,
    ) -> impl PinInit<Self, Error> {
        try_pin_init!(Self {
            data <- data,
            inner <- Opaque::try_ffi_init(move |slot: *mut bindings::miscdevice| {
                unsafe { slot.write(opts.into_raw::<T>()) };
                to_result(unsafe { bindings::misc_register(slot) })
            }),
        })
    }
    pub fn as_raw(&self) -> *mut bindings::miscdevice {
        self.inner.get()
    }
    pub fn device(&self) -> &Device {
        unsafe { Device::as_ref((*self.as_raw()).this_device) }
    }
}
impl<T> Deref for MiscDeviceRegistration<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        &self.data
    }
}
#[pinned_drop]
impl<T> PinnedDrop for MiscDeviceRegistration<T> {
    fn drop(self: Pin<&mut Self>) {
        unsafe { bindings::misc_deregister(self.inner.get()) };
    }
}
#[vtable]
pub trait MiscDevice: Sized + Sync + 'static {
    type File: ForeignOwnable + Send + Sync + 'static;
    fn open(_file: &File, _misc: &MiscDeviceRegistration<Self>) -> Result<Self::File>;
    fn release(device: Self::File, _file: &File) {
        drop(device);
    }
    fn mmap(
        _device: <Self::File as ForeignOwnable>::Borrowed<'_>,
        _file: &File,
        _vma: &VmaNew,
    ) -> Result {
        kernel::build_error(VTABLE_DEFAULT_ERROR)
    }
    fn llseek(
        _device: <Self::File as ForeignOwnable>::Borrowed<'_>,
        _file: &LocalFile,
        _offset: Offset,
        _whence: c_int,
    ) -> Result<Offset> {
        kernel::build_error(VTABLE_DEFAULT_ERROR)
    }
    fn read_iter(_kiocb: Kiocb<'_, Self::File>, _iov: &mut IovIter) -> Result<usize> {
        kernel::build_error(VTABLE_DEFAULT_ERROR)
    }
    fn write_iter(_kiocb: Kiocb<'_, Self::File>, _iov: &mut IovIter) -> Result<usize> {
        kernel::build_error(VTABLE_DEFAULT_ERROR)
    }
    fn ioctl(
        _device: <Self::File as ForeignOwnable>::Borrowed<'_>,
        _file: &File,
        _cmd: u32,
        _arg: usize,
    ) -> Result<isize> {
        kernel::build_error!(VTABLE_DEFAULT_ERROR)
    }
    #[cfg(CONFIG_COMPAT)]
    fn compat_ioctl(
        _device: <Self::File as ForeignOwnable>::Borrowed<'_>,
        _file: &File,
        _cmd: u32,
        _arg: usize,
    ) -> Result<isize> {
        kernel::build_error!(VTABLE_DEFAULT_ERROR)
    }
    fn show_fdinfo(
        _device: <Self::File as ForeignOwnable>::Borrowed<'_>,
        _m: &SeqFile,
        _file: &File,
    ) {
        kernel::build_error!(VTABLE_DEFAULT_ERROR)
    }
}
pub struct Kiocb<'a, T> {
    inner: NonNull<bindings::kiocb>,
    _phantom: PhantomData<&'a T>,
}
impl<'a, T: ForeignOwnable> Kiocb<'a, T> {
    pub fn private_data(&self) -> <T as ForeignOwnable>::Borrowed<'a> {
        let private = unsafe { (*(*self.inner.as_ptr()).ki_filp).private_data };
        unsafe { <T as ForeignOwnable>::borrow(private) }
    }
    pub fn ki_pos(&self) -> Offset {
        unsafe { (*self.inner.as_ptr()).ki_pos }
    }
    pub fn ki_pos_mut(&mut self) -> &mut Offset {
        unsafe { &mut (*self.inner.as_ptr()).ki_pos }
    }
}
pub struct IovIter {
    inner: Opaque<bindings::iov_iter>,
}
impl IovIter {
    pub fn as_raw(&self) -> *mut bindings::iov_iter {
        self.inner.get()
    }
    pub fn copy_from_iter(&mut self, buf: &mut [u8]) -> usize {
        unsafe {
            bindings::_copy_from_iter(
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len(),
                self.inner.get(),
            )
        }
    }
    pub fn copy_to_iter<T: AsBytes>(&mut self, value: &T) -> Result<()> {
        let len = size_of::<T>();
        let res = unsafe {
            bindings::_copy_to_iter((value as *const T).cast::<c_void>(), len, self.inner.get())
        };
        if res == len {
            Ok(())
        } else {
            Err(EFAULT)
        }
    }
}
pub struct MiscdeviceVTable<T: MiscDevice>(PhantomData<T>);
impl<T: MiscDevice> MiscdeviceVTable<T> {
    pub unsafe extern "C" fn open(
        inode: *mut bindings::inode,
        raw_file: *mut bindings::file,
    ) -> c_int {
        let ret = unsafe { bindings::generic_file_open(inode, raw_file) };
        if ret != 0 {
            return ret;
        }
        let misc_ptr = unsafe { (*raw_file).private_data };
        let misc = unsafe { &*container_of!(misc_ptr, MiscDeviceRegistration<T>, inner) };
        let file = unsafe { File::from_raw_file(raw_file) };
        let ptr = match T::open(file, misc) {
            Ok(ptr) => ptr,
            Err(err) => return err.to_errno(),
        };
        unsafe { (*raw_file).private_data = ptr.into_foreign().cast_mut() };
        0
    }
    pub unsafe extern "C" fn release(
        _inode: *mut bindings::inode,
        file: *mut bindings::file,
    ) -> c_int {
        let private = unsafe { (*file).private_data };
        let ptr = unsafe { <T::File as ForeignOwnable>::from_foreign(private) };
        T::release(ptr, unsafe { File::from_raw_file(file) });
        0
    }
    unsafe extern "C" fn mmap(
        file: *mut bindings::file,
        vma: *mut bindings::vm_area_struct,
    ) -> c_int {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::File as ForeignOwnable>::borrow(private) };
        let area = unsafe { VmaNew::from_raw(vma) };
        let file = unsafe { File::from_raw_file(file) };
        match T::mmap(device, file, area) {
            Ok(()) => 0,
            Err(err) => err.to_errno(),
        }
    }
    unsafe extern "C" fn llseek(
        file: *mut bindings::file,
        offset: Offset,
        whence: c_int,
    ) -> Offset {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::File as ForeignOwnable>::borrow(private) };
        let file = unsafe { LocalFile::from_raw_file(file) };
        match T::llseek(device, file, offset, whence) {
            Ok(res) => res,
            Err(err) => err.to_errno() as Offset,
        }
    }
    unsafe extern "C" fn read_iter(
        kiocb: *mut bindings::kiocb,
        iter: *mut bindings::iov_iter,
    ) -> isize {
        let kiocb = Kiocb {
            inner: unsafe { NonNull::new_unchecked(kiocb) },
            _phantom: PhantomData,
        };
        let iov = unsafe { &mut *iter.cast::<IovIter>() };
        match T::read_iter(kiocb, iov) {
            Ok(res) => res as isize,
            Err(err) => err.to_errno() as isize,
        }
    }
    unsafe extern "C" fn write_iter(
        kiocb: *mut bindings::kiocb,
        iter: *mut bindings::iov_iter,
    ) -> isize {
        let kiocb = Kiocb {
            inner: unsafe { NonNull::new_unchecked(kiocb) },
            _phantom: PhantomData,
        };
        let iov = unsafe { &mut *iter.cast::<IovIter>() };
        match T::write_iter(kiocb, iov) {
            Ok(res) => res as isize,
            Err(err) => err.to_errno() as isize,
        }
    }
    pub unsafe extern "C" fn ioctl(file: *mut bindings::file, cmd: c_uint, arg: c_ulong) -> c_long {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::File as ForeignOwnable>::borrow(private) };
        let file = unsafe { File::from_raw_file(file) };
        match T::ioctl(device, file, cmd, arg) {
            Ok(ret) => ret as c_long,
            Err(err) => err.to_errno() as c_long,
        }
    }
    #[cfg(CONFIG_COMPAT)]
    unsafe extern "C" fn compat_ioctl(
        file: *mut bindings::file,
        cmd: c_uint,
        arg: c_ulong,
    ) -> c_long {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::File as ForeignOwnable>::borrow(private) };
        let file = unsafe { File::from_raw_file(file) };
        match T::compat_ioctl(device, file, cmd, arg) {
            Ok(ret) => ret as c_long,
            Err(err) => err.to_errno() as c_long,
        }
    }
    unsafe extern "C" fn show_fdinfo(seq_file: *mut bindings::seq_file, file: *mut bindings::file) {
        let private = unsafe { (*file).private_data };
        let device = unsafe { <T::File as ForeignOwnable>::borrow(private) };
        let file = unsafe { File::from_raw_file(file) };
        let m = unsafe { SeqFile::from_raw(seq_file) };
        T::show_fdinfo(device, m, file);
    }
    const VTABLE: bindings::file_operations = bindings::file_operations {
        open: Some(bindings::kcfi_misc_open),
        release: Some(bindings::kcfi_misc_release),
        mmap: if T::HAS_MMAP { Some(Self::mmap) } else { None },
        llseek: if T::HAS_LLSEEK {
            Some(Self::llseek)
        } else {
            None
        },
        read_iter: if T::HAS_READ_ITER {
            Some(Self::read_iter)
        } else {
            None
        },
        write_iter: if T::HAS_WRITE_ITER {
            Some(Self::write_iter)
        } else {
            None
        },
        unlocked_ioctl: if T::HAS_IOCTL {
            Some(bindings::kcfi_misc_ioctl)
        } else {
            None
        },
        #[cfg(CONFIG_COMPAT)]
        compat_ioctl: if T::HAS_COMPAT_IOCTL {
            Some(Self::compat_ioctl)
        } else if T::HAS_IOCTL {
            Some(bindings::compat_ptr_ioctl)
        } else {
            None
        },
        show_fdinfo: if T::HAS_SHOW_FDINFO {
            Some(Self::show_fdinfo)
        } else {
            None
        },
        ..unsafe { MaybeUninit::zeroed().assume_init() }
    };
    const fn build() -> &'static bindings::file_operations {
        &Self::VTABLE
    }
}
