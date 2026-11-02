// SPDX-License-Identifier: GPL-2.0

#![no_std]
#![feature(arbitrary_self_types)]
#![feature(coerce_unsized)]
#![feature(dispatch_from_dyn)]
#![feature(inline_const)]
#![feature(lint_reasons)]
#![feature(unsize)]
#![allow(
    dead_code,
    missing_docs,
    unreachable_pub,
    unused_attributes,
    unused_imports
)]

extern crate self as kernel;

pub use bindings;
pub extern crate bindings as uapi;
pub mod ffi {
    pub use core::ffi::{
        c_char, c_int, c_longlong, c_schar, c_short, c_uchar, c_uint, c_ulonglong, c_ushort, c_void,
    };
    #[allow(non_camel_case_types)]
    pub type c_long = isize;
    #[allow(non_camel_case_types)]
    pub type c_ulong = usize;
}
pub use macros;

#[macro_use]
pub mod build_error {

    #![no_std]

    #[inline(never)]
    #[cold]
    #[export_name = "rust_build_error"]
    #[track_caller]
    pub const fn build_error(msg: &'static str) -> ! {
        panic!("{}", msg);
    }
}
pub use build_error::build_error;

pub mod build_assert {

    #[macro_export]
    macro_rules! build_error {
        () => {{
            $crate::build_error("")
        }};
        ($msg:expr) => {{
            $crate::build_error($msg)
        }};
    }

    #[macro_export]
    macro_rules! build_assert {
        ($cond:expr $(,)?) => {{
            if !$cond {
                $crate::build_error(concat!("assertion failed: ", stringify!($cond)));
            }
        }};
        ($cond:expr, $msg:expr) => {{
            if !$cond {
                $crate::build_error($msg);
            }
        }};
    }
}
pub mod ioctl {

    #![allow(non_snake_case)]

    use crate::build_assert;

    #[inline(always)]
    const fn _IOC(dir: u32, ty: u32, nr: u32, size: usize) -> u32 {
        build_assert!(dir <= uapi::_IOC_DIRMASK);
        build_assert!(ty <= uapi::_IOC_TYPEMASK);
        build_assert!(nr <= uapi::_IOC_NRMASK);
        build_assert!(size <= (uapi::_IOC_SIZEMASK as usize));

        (dir << uapi::_IOC_DIRSHIFT)
            | (ty << uapi::_IOC_TYPESHIFT)
            | (nr << uapi::_IOC_NRSHIFT)
            | ((size as u32) << uapi::_IOC_SIZESHIFT)
    }

    #[inline(always)]
    pub const fn _IO(ty: u32, nr: u32) -> u32 {
        _IOC(uapi::_IOC_NONE, ty, nr, 0)
    }

    #[inline(always)]
    pub const fn _IOR<T>(ty: u32, nr: u32) -> u32 {
        _IOC(uapi::_IOC_READ, ty, nr, core::mem::size_of::<T>())
    }

    #[inline(always)]
    pub const fn _IOW<T>(ty: u32, nr: u32) -> u32 {
        _IOC(uapi::_IOC_WRITE, ty, nr, core::mem::size_of::<T>())
    }

    #[inline(always)]
    pub const fn _IOWR<T>(ty: u32, nr: u32) -> u32 {
        _IOC(
            uapi::_IOC_READ | uapi::_IOC_WRITE,
            ty,
            nr,
            core::mem::size_of::<T>(),
        )
    }

    pub const fn _IOC_DIR(nr: u32) -> u32 {
        (nr >> uapi::_IOC_DIRSHIFT) & uapi::_IOC_DIRMASK
    }

    pub const fn _IOC_TYPE(nr: u32) -> u32 {
        (nr >> uapi::_IOC_TYPESHIFT) & uapi::_IOC_TYPEMASK
    }

    pub const fn _IOC_NR(nr: u32) -> u32 {
        (nr >> uapi::_IOC_NRSHIFT) & uapi::_IOC_NRMASK
    }

    pub const fn _IOC_SIZE(nr: u32) -> usize {
        ((nr >> uapi::_IOC_SIZESHIFT) & uapi::_IOC_SIZEMASK) as usize
    }
}
pub mod print {

    use core::{
        ffi::{c_char, c_void},
        fmt,
    };

    use crate::str::RawFormatter;

    #[cfg(CONFIG_PRINTK)]
    use crate::bindings;

    #[no_mangle]
    unsafe extern "C" fn rust_fmt_argument(
        buf: *mut c_char,
        end: *mut c_char,
        ptr: *const c_void,
    ) -> *mut c_char {
        use fmt::Write;
        let mut w = unsafe { RawFormatter::from_ptrs(buf.cast(), end.cast()) };
        let _ = w.write_fmt(unsafe { *(ptr as *const fmt::Arguments<'_>) });
        w.pos().cast()
    }

    #[doc(hidden)]
    pub mod format_strings {
        use crate::bindings;

        const LENGTH_PREFIX: usize = 2;

        pub const LENGTH: usize = 10;

        const fn generate(is_cont: bool, prefix: &[u8; 3]) -> [u8; LENGTH] {
            assert!(prefix[0] == b'\x01');
            if is_cont {
                assert!(prefix[1] == b'c');
            } else {
                assert!(prefix[1] >= b'0' && prefix[1] <= b'7');
            }
            assert!(prefix[2] == b'\x00');

            let suffix: &[u8; LENGTH - LENGTH_PREFIX] = if is_cont {
                b"%pA\0\0\0\0\0"
            } else {
                b"%s: %pA\0"
            };

            [
                prefix[0], prefix[1], suffix[0], suffix[1], suffix[2], suffix[3], suffix[4],
                suffix[5], suffix[6], suffix[7],
            ]
        }

        pub static EMERG: [u8; LENGTH] = generate(false, bindings::KERN_EMERG);
        pub static ALERT: [u8; LENGTH] = generate(false, bindings::KERN_ALERT);
        pub static CRIT: [u8; LENGTH] = generate(false, bindings::KERN_CRIT);
        pub static ERR: [u8; LENGTH] = generate(false, bindings::KERN_ERR);
        pub static WARNING: [u8; LENGTH] = generate(false, bindings::KERN_WARNING);
        pub static NOTICE: [u8; LENGTH] = generate(false, bindings::KERN_NOTICE);
        pub static INFO: [u8; LENGTH] = generate(false, bindings::KERN_INFO);
        pub static DEBUG: [u8; LENGTH] = generate(false, bindings::KERN_DEBUG);
        pub static CONT: [u8; LENGTH] = generate(true, bindings::KERN_CONT);
    }

    #[doc(hidden)]
    #[cfg_attr(not(CONFIG_PRINTK), allow(unused_variables))]
    pub unsafe fn call_printk(
        format_string: &[u8; format_strings::LENGTH],
        module_name: &[u8],
        args: fmt::Arguments<'_>,
    ) {
        #[cfg(CONFIG_PRINTK)]
        unsafe {
            bindings::_printk(
                format_string.as_ptr() as _,
                module_name.as_ptr(),
                &args as *const _ as *const c_void,
            );
        }
    }

    #[doc(hidden)]
    #[cfg_attr(not(CONFIG_PRINTK), allow(unused_variables))]
    pub fn call_printk_cont(args: fmt::Arguments<'_>) {
        #[cfg(CONFIG_PRINTK)]
        unsafe {
            bindings::_printk(
                format_strings::CONT.as_ptr() as _,
                &args as *const _ as *const c_void,
            );
        }
    }

    #[doc(hidden)]
    #[cfg(not(testlib))]
    #[macro_export]
    #[allow(clippy::crate_in_macro_def)]
    macro_rules! print_macro (
    ($format_string:path, false, $($arg:tt)+) => (
        match format_args!($($arg)+) {
            args => unsafe {
                $crate::print::call_printk(
                    &$format_string,
                    crate::__LOG_PREFIX,
                    args,
                );
            }
        }
    );

    ($format_string:path, true, $($arg:tt)+) => (
        $crate::print::call_printk_cont(
            format_args!($($arg)+),
        );
    );
);

    #[cfg(testlib)]
    #[macro_export]
    macro_rules! print_macro (
    ($format_string:path, $e:expr, $($arg:tt)+) => (
        ()
    );
);

    #[macro_export]
    macro_rules! pr_emerg (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::EMERG, false, $($arg)*)
    )
);

    #[macro_export]
    macro_rules! pr_alert (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::ALERT, false, $($arg)*)
    )
);

    #[macro_export]
    macro_rules! pr_crit (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::CRIT, false, $($arg)*)
    )
);

    #[macro_export]
    macro_rules! pr_err (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::ERR, false, $($arg)*)
    )
);

    #[macro_export]
    macro_rules! pr_warn (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::WARNING, false, $($arg)*)
    )
);

    #[macro_export]
    macro_rules! pr_notice (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::NOTICE, false, $($arg)*)
    )
);

    #[macro_export]
    #[doc(alias = "print")]
    macro_rules! pr_info (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::INFO, false, $($arg)*)
    )
);

    #[macro_export]
    #[doc(alias = "print")]
    macro_rules! pr_debug (
    ($($arg:tt)*) => (
        if cfg!(debug_assertions) {
            $crate::print_macro!($crate::print::format_strings::DEBUG, false, $($arg)*)
        }
    )
);

    #[macro_export]
    macro_rules! pr_cont (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::CONT, true, $($arg)*)
    )
);
}
pub mod static_assert {

    #[macro_export]
    macro_rules! static_assert {
        ($condition:expr) => {
            const _: () = core::assert!($condition);
        };
    }
}

pub mod anon_inodes {
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
            match T::ioctl(device, file, cmd, arg as usize) {
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
            match T::compat_ioctl(device, file, cmd, arg as usize) {
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
}

pub mod device {
    use crate::{
        bindings,
        types::{ARef, Opaque},
    };
    use core::ptr;
    #[repr(transparent)]
    pub struct Device(Opaque<bindings::device>);
    impl Device {
        pub unsafe fn get_device(ptr: *mut bindings::device) -> ARef<Self> {
            unsafe { Self::as_ref(ptr) }.into()
        }
        pub fn as_raw(&self) -> *mut bindings::device {
            self.0.get()
        }
        pub unsafe fn from_raw<'a>(ptr: *mut bindings::device) -> &'a Self {
            unsafe { &*ptr.cast() }
        }
        pub unsafe fn as_ref<'a>(ptr: *mut bindings::device) -> &'a Self {
            unsafe { &*ptr.cast() }
        }
    }
    impl PartialEq for Device {
        fn eq(&self, other: &Self) -> bool {
            self.as_raw() == other.as_raw()
        }
    }
    impl Eq for Device {}
    unsafe impl crate::types::AlwaysRefCounted for Device {
        fn inc_ref(&self) {
            unsafe { bindings::get_device(self.as_raw()) };
        }
        unsafe fn dec_ref(obj: ptr::NonNull<Self>) {
            unsafe { bindings::put_device(obj.cast().as_ptr()) }
        }
    }
    unsafe impl Send for Device {}
    unsafe impl Sync for Device {}
}

pub mod dma {
    use crate::{bindings, build_error};
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    #[repr(u32)]
    pub enum DataDirection {
        Bidirectional = Self::const_cast(bindings::dma_data_direction_DMA_BIDIRECTIONAL),
        ToDevice = Self::const_cast(bindings::dma_data_direction_DMA_TO_DEVICE),
        FromDevice = Self::const_cast(bindings::dma_data_direction_DMA_FROM_DEVICE),
        None = Self::const_cast(bindings::dma_data_direction_DMA_NONE),
    }
    impl DataDirection {
        const fn const_cast(val: bindings::dma_data_direction) -> u32 {
            let wide_val = val as i128;
            if wide_val < 0 || wide_val > u32::MAX as i128 {
                build_error!("C enum value is out of bounds for the target type `u32`.");
            }
            wide_val as u32
        }
    }
    impl From<DataDirection> for bindings::dma_data_direction {
        fn from(direction: DataDirection) -> Self {
            direction as u32 as Self
        }
    }
}

pub mod error {
    use crate::{
        alloc::{layout::LayoutError, AllocError},
        bindings,
        str::CStr,
    };
    use core::{
        fmt,
        num::{NonZeroI32, TryFromIntError},
        str::Utf8Error,
    };
    pub mod code {
        macro_rules! declare_err {
        ($err:tt $(,)? $($doc:expr),+) => {
            $(#[doc = $doc])*
            pub const $err: super::Error =
                match super::Error::try_from_errno(-(crate::bindings::$err as i32)) {
                    Some(err) => err,
                    None => panic!("Invalid errno in `declare_err!`"),
                };
        };
    }
        declare_err!(ENOENT, "No such file or directory.");
        declare_err!(EBADF, "Bad file number.");
        declare_err!(ENOMEM, "Out of memory.");
        declare_err!(EACCES, "Permission denied.");
        declare_err!(EFAULT, "Bad address.");
        declare_err!(EBUSY, "Device or resource busy.");
        declare_err!(EEXIST, "File exists.");
        declare_err!(ENODEV, "No such device.");
        declare_err!(EINVAL, "Invalid argument.");
        declare_err!(ENOTTY, "Inappropriate ioctl for device.");
        declare_err!(ENOTSUPP, "Operation is not supported.");
    }
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(transparent)]
    pub struct Error(NonZeroI32);
    impl Error {
        pub fn from_errno(errno: crate::ffi::c_int) -> Error {
            if errno < -(bindings::MAX_ERRNO as i32) || errno >= 0 {
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
}

pub mod impl_flags {
    #[macro_export]
    macro_rules! impl_flags {
    (
        $(#[$outer_flags:meta])*
        $vis_flags:vis struct $flags:ident($ty:ty);
        $(#[$outer_flag:meta])*
        $vis_flag:vis enum $flag:ident {
            $(
                $(#[$inner_flag:meta])*
                $name:ident = $value:expr
            ),+ $( , )?
        }
    ) => {
        $(#[$outer_flags])*
        #[repr(transparent)]
        $vis_flags struct $flags($ty);
        $(#[$outer_flag])*
        #[repr($ty)]
        $vis_flag enum $flag {
            $(
                $(#[$inner_flag])*
                $name = $value
            ),+
        }
        impl ::core::convert::From<$flag> for $flags {
            #[inline]
            fn from(value: $flag) -> Self {
                Self(value as $ty)
            }
        }
        impl ::core::convert::From<$flags> for $ty {
            #[inline]
            fn from(value: $flags) -> Self {
                value.0
            }
        }
    };
}
}

pub mod miscdevice {
    use crate::{
        bindings, container_of,
        device::Device,
        error::{to_result, Error, Result, VTABLE_DEFAULT_ERROR},
        ffi::{c_int, c_long, c_uint, c_ulong, c_void},
        fs::file::Offset,
        fs::{File, LocalFile},
        prelude::*,
        str::CStr,
        types::{ForeignOwnable, Opaque},
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

        fn ioctl(
            _device: <Self::File as ForeignOwnable>::Borrowed<'_>,
            _file: &File,
            _cmd: u32,
            _arg: usize,
        ) -> Result<isize> {
            kernel::build_error!(VTABLE_DEFAULT_ERROR)
        }
    }

    pub struct MiscdeviceVTable<T: MiscDevice>(PhantomData<T>);

    impl<T: MiscDevice> MiscdeviceVTable<T> {
        pub unsafe extern "C" fn open(
            inode: *mut bindings::inode,
            raw_file: *mut bindings::file,
        ) -> c_int {
            let misc_ptr = unsafe { (*raw_file).private_data };
            let file = unsafe { File::from_raw_file(raw_file) };
            let misc = unsafe {
                container_of!(
                    misc_ptr.cast::<Opaque<bindings::miscdevice>>(),
                    MiscDeviceRegistration<T>,
                    inner
                )
            };
            let ptr = match T::open(file, unsafe { &*misc }) {
                Ok(ptr) => ptr,
                Err(err) => return err.to_errno(),
            };
            unsafe { (*raw_file).private_data = ptr.into_foreign().cast_mut() };
            unsafe { bindings::stream_open(inode, raw_file) }
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

        pub unsafe extern "C" fn ioctl(
            file: *mut bindings::file,
            cmd: c_uint,
            arg: c_ulong,
        ) -> c_long {
            let private = unsafe { (*file).private_data };
            let device = unsafe { <T::File as ForeignOwnable>::borrow(private) };
            let file = unsafe { File::from_raw_file(file) };
            match T::ioctl(device, file, cmd, arg as usize) {
                Ok(ret) => ret as c_long,
                Err(err) => err.to_errno() as c_long,
            }
        }

        const VTABLE: bindings::file_operations = bindings::file_operations {
            owner: core::ptr::null_mut(),
            open: Some(bindings::kcfi_misc_open),
            release: Some(bindings::kcfi_misc_release),
            unlocked_ioctl: if T::HAS_IOCTL {
                Some(bindings::kcfi_misc_ioctl)
            } else {
                None
            },
            #[cfg(CONFIG_COMPAT)]
            compat_ioctl: if T::HAS_IOCTL {
                Some(bindings::compat_ptr_ioctl)
            } else {
                None
            },
            ..unsafe { MaybeUninit::zeroed().assume_init() }
        };

        const fn build() -> &'static bindings::file_operations {
            &Self::VTABLE
        }
    }
}

pub mod module_param {
    use crate::ffi::{c_char, c_int, c_void};
    use crate::page::PAGE_SIZE;
    use crate::prelude::*;
    use crate::str::{BStr, RawFormatter};
    use bindings;
    use core::fmt::Write;
    use kernel::sync::SetOnce;
    #[repr(transparent)]
    #[doc(hidden)]
    pub struct KernelParam(bindings::kernel_param);
    impl KernelParam {
        #[doc(hidden)]
        pub const fn new(val: bindings::kernel_param) -> Self {
            Self(val)
        }
    }
    unsafe impl Sync for KernelParam {}
    pub trait ModuleParam: Sized + Copy {
        fn try_from_param_arg(arg: &BStr) -> Result<Self>;
    }
    unsafe extern "C" fn set_param<T>(
        val: *const c_char,
        param: *const bindings::kernel_param,
    ) -> c_int
    where
        T: ModuleParam,
    {
        if val.is_null() {
            crate::pr_warn!("Null pointer passed to `module_param::set_param`");
            return EINVAL.to_errno();
        }
        let arg = unsafe { CStr::from_char_ptr(val) };
        crate::error::from_result(|| {
            let new_value = T::try_from_param_arg(arg)?;
            let container = unsafe { &*((*param).__bindgen_anon_1.arg.cast::<SetOnce<T>>()) };
            container
                .populate(new_value)
                .then_some(0)
                .ok_or(kernel::error::code::EEXIST)
        })
    }
    macro_rules! impl_int_module_param {
        ($ty:ident) => {
            impl ModuleParam for $ty {
                fn try_from_param_arg(arg: &BStr) -> Result<Self> {
                    <$ty as crate::str::parse_int::ParseInt>::from_str(arg)
                }
            }
        };
    }
    impl_int_module_param!(usize);
    unsafe extern "C" fn get_param<T>(
        buf: *mut c_char,
        param: *const bindings::kernel_param,
    ) -> c_int
    where
        T: core::fmt::Display,
    {
        if buf.is_null() {
            crate::pr_warn!("Null pointer passed to `module_param::get_param`");
            return EINVAL.to_errno();
        }
        let mut writer = unsafe { RawFormatter::from_buffer(buf.cast(), PAGE_SIZE - 1) };
        let container = unsafe { &*((*param).__bindgen_anon_1.arg.cast::<ModuleParamAccess<T>>()) };
        let _ = writeln!(writer, "{}", container.value());
        unsafe {
            *writer.pos() = 0;
        }
        writer.bytes_written() as c_int
    }
    pub struct ModuleParamAccess<T> {
        value: SetOnce<T>,
        default: T,
    }
    unsafe impl<T: Sync> Sync for ModuleParamAccess<T> {}
    impl<T> ModuleParamAccess<T> {
        #[doc(hidden)]
        pub const fn new(default: T) -> Self {
            Self {
                value: SetOnce::new(),
                default,
            }
        }
        pub fn value(&self) -> &T {
            self.value.as_ref().unwrap_or(&self.default)
        }
        pub const fn as_void_ptr(&self) -> *mut c_void {
            core::ptr::from_ref(self).cast_mut().cast()
        }
    }
    #[doc(hidden)]
    #[no_mangle]
    pub unsafe extern "C" fn __rust_param_set_usize(
        val: *const c_char,
        param: *const bindings::kernel_param,
    ) -> c_int {
        unsafe { set_param::<usize>(val, param) }
    }
    #[doc(hidden)]
    #[no_mangle]
    pub unsafe extern "C" fn __rust_param_get_usize(
        buf: *mut c_char,
        param: *const bindings::kernel_param,
    ) -> c_int {
        unsafe { get_param::<usize>(buf, param) }
    }
    #[doc(hidden)]
    pub static PARAM_OPS_USIZE: crate::bindings::kernel_param_ops =
        crate::bindings::kernel_param_ops {
            flags: 0,
            set: Some(crate::bindings::kcfi_param_set_usize),
            get: Some(crate::bindings::kcfi_param_get_usize),
            free: None,
        };
}

pub mod page {
    use crate::{
        alloc::{AllocError, Flags},
        bindings,
        error::code::*,
        error::Result,
        uaccess::UserSliceReader,
    };
    use core::ptr::{self, NonNull};
    pub const PAGE_SHIFT: usize = bindings::PAGE_SHIFT as usize;
    pub const PAGE_SIZE: usize = bindings::PAGE_SIZE;
    pub const PAGE_MASK: usize = !(PAGE_SIZE - 1);
    pub const fn page_align(addr: usize) -> usize {
        (addr + (PAGE_SIZE - 1)) & PAGE_MASK
    }
    pub fn page_align_down(addr: usize) -> usize {
        addr & PAGE_MASK
    }
    pub fn offset_in_page(addr: usize) -> usize {
        addr & !PAGE_MASK
    }
    pub fn is_page_aligned(addr: usize) -> bool {
        offset_in_page(addr) == 0
    }
    pub struct Page {
        page: NonNull<bindings::page>,
    }
    unsafe impl Send for Page {}
    unsafe impl Sync for Page {}
    impl Page {
        #[inline]
        pub fn alloc_page(flags: Flags) -> Result<Self, AllocError> {
            let page = unsafe { bindings::alloc_pages(flags.as_raw(), 0) };
            let page = NonNull::new(page).ok_or(AllocError)?;
            Ok(Self { page })
        }
        pub fn as_ptr(&self) -> *mut bindings::page {
            self.page.as_ptr()
        }
    }
    impl Drop for Page {
        #[inline]
        fn drop(&mut self) {
            unsafe { bindings::__free_pages(self.page.as_ptr(), 0) };
        }
    }
}

pub mod page_size_compat {
    use kernel::page;
    #[inline(always)]
    pub fn __page_shift() -> usize {
        page::PAGE_SHIFT
    }
    #[inline(always)]
    pub fn __page_size() -> usize {
        1usize << __page_shift()
    }
    #[inline(always)]
    pub fn __page_mask() -> usize {
        !(__page_size() - 1)
    }
    #[inline(always)]
    pub fn __page_align(addr: usize) -> usize {
        let page_size = __page_size();
        let mask = !(page_size - 1);
        (addr + (page_size - 1)) & mask
    }
    #[inline(always)]
    pub fn __page_align_down(addr: usize) -> usize {
        addr & __page_mask()
    }
}

pub mod prelude {
    pub use super::build_assert;
    pub use super::current;
    pub use super::error::{code::*, Error, Result};
    pub use super::init::{InPlaceInit, InPlaceWrite, Init, PinInit};
    pub use super::static_assert;
    pub use super::{init, pin_init, try_init, try_pin_init};
    #[doc(no_inline)]
    pub use super::{pr_alert, pr_crit, pr_debug, pr_emerg, pr_err, pr_info, pr_notice, pr_warn};
    pub use super::{str::CStr, ThisModule};
    pub use crate::alloc::{flags::*, Box, KBox, KVVec, KVec, Vec};
    pub use crate::ffi;
    pub use crate::sync::{new_mutex, Mutex};
    #[doc(no_inline)]
    pub use core::pin::Pin;
    #[doc(no_inline)]
    pub use macros::{module, pin_data, pinned_drop, vtable, Zeroable};
}

pub mod seq_file {
    use crate::{bindings, c_str, str::RawFormatter, types::NotThreadSafe, types::Opaque};
    use core::fmt::Write;
    #[repr(transparent)]
    pub struct SeqFile {
        inner: Opaque<bindings::seq_file>,
        _not_send: NotThreadSafe,
    }
    impl SeqFile {
        pub unsafe fn from_raw<'a>(ptr: *mut bindings::seq_file) -> &'a SeqFile {
            unsafe { &*ptr.cast() }
        }
        pub fn call_printf(&self, args: core::fmt::Arguments<'_>) {
            let mut buf = [0u8; 256];
            let mut w = unsafe {
                RawFormatter::from_ptrs(buf.as_mut_ptr(), buf.as_mut_ptr().add(buf.len() - 1))
            };
            let _ = w.write_fmt(args);
            let pos = w.pos();
            unsafe { *pos = 0 };
            unsafe {
                bindings::seq_printf(
                    self.inner.get(),
                    c_str!("%s").as_char_ptr(),
                    buf.as_ptr().cast::<core::ffi::c_char>(),
                );
            }
        }
    }
    #[macro_export]
    macro_rules! seq_print {
    ($m:expr, $($arg:tt)+) => (
        $m.call_printf(format_args!($($arg)+))
    );
}
    pub use seq_print;
}

pub mod task {
    use crate::{
        bindings,
        ffi::{c_int, c_long, c_uint},
        types::{ARef, NotThreadSafe, Opaque},
    };
    use core::{
        cmp::{Eq, PartialEq},
        ops::Deref,
        ptr,
    };
    #[macro_export]
    macro_rules! current {
        () => {
            unsafe { &*$crate::task::Task::current() }
        };
    }
    #[repr(transparent)]
    pub struct Task(pub Opaque<bindings::task_struct>);
    unsafe impl Send for Task {}
    unsafe impl Sync for Task {}
    #[repr(transparent)]
    pub struct CurrentTask(Task, NotThreadSafe);
    impl Deref for CurrentTask {
        type Target = Task;
        #[inline]
        fn deref(&self) -> &Task {
            &self.0
        }
    }
    pub type Pid = bindings::pid_t;
    #[derive(Copy, Clone)]
    pub struct Kuid {
        kuid: bindings::kuid_t,
    }
    impl Task {
        #[inline]
        pub fn current_raw() -> *mut bindings::task_struct {
            unsafe { bindings::get_current() }
        }
        pub unsafe fn current() -> impl Deref<Target = CurrentTask> {
            struct TaskRef {
                task: *const CurrentTask,
            }
            impl Deref for TaskRef {
                type Target = CurrentTask;
                fn deref(&self) -> &Self::Target {
                    unsafe { &*self.task }
                }
            }
            TaskRef {
                task: Task::current_raw().cast(),
            }
        }
        #[inline]
        pub fn as_ptr(&self) -> *mut bindings::task_struct {
            self.0.get()
        }
        pub fn group_leader(&self) -> &Task {
            let ptr = unsafe { *ptr::addr_of!((*self.as_ptr()).group_leader) };
            unsafe { &*ptr.cast() }
        }
        pub fn pid(&self) -> Pid {
            unsafe { *ptr::addr_of!((*self.as_ptr()).pid) }
        }
        #[inline]
        pub fn has_mm(&self) -> bool {
            let mm = unsafe { core::ptr::addr_of!((*self.as_ptr()).mm).read_volatile() };
            !mm.is_null()
        }
    }
    impl CurrentTask {}
    unsafe impl crate::types::AlwaysRefCounted for Task {
        fn inc_ref(&self) {
            unsafe { bindings::get_task_struct(self.as_ptr()) };
        }
        unsafe fn dec_ref(obj: ptr::NonNull<Self>) {
            unsafe { bindings::put_task_struct(obj.cast().as_ptr()) }
        }
    }
    impl PartialEq for Task {
        #[inline]
        fn eq(&self, other: &Self) -> bool {
            ptr::eq(self.as_ptr(), other.as_ptr())
        }
    }
    impl Eq for Task {}
}

pub mod types {
    use crate::init::{self, PinInit};
    use core::{
        cell::UnsafeCell,
        marker::{PhantomData, PhantomPinned},
        mem::{ManuallyDrop, MaybeUninit},
        ops::{Deref, DerefMut},
        ptr::NonNull,
    };
    pub trait ForeignOwnable: Sized {
        type Borrowed<'a>;
        fn into_foreign(self) -> *const crate::ffi::c_void;
        unsafe fn borrow<'a>(ptr: *const crate::ffi::c_void) -> Self::Borrowed<'a>;
        unsafe fn from_foreign(ptr: *const crate::ffi::c_void) -> Self;
        unsafe fn try_from_foreign(ptr: *const crate::ffi::c_void) -> Option<Self> {
            if ptr.is_null() {
                None
            } else {
                unsafe { Some(Self::from_foreign(ptr)) }
            }
        }
    }
    impl ForeignOwnable for () {
        type Borrowed<'a> = ();
        fn into_foreign(self) -> *const crate::ffi::c_void {
            core::ptr::NonNull::dangling().as_ptr()
        }
        unsafe fn borrow<'a>(_: *const crate::ffi::c_void) -> Self::Borrowed<'a> {}
        unsafe fn from_foreign(_: *const crate::ffi::c_void) -> Self {}
    }
    pub struct ScopeGuard<T, F: FnOnce(T)>(Option<(T, F)>);
    impl<T, F: FnOnce(T)> ScopeGuard<T, F> {
        pub fn new_with_data(data: T, cleanup_func: F) -> Self {
            Self(Some((data, cleanup_func)))
        }
        pub fn dismiss(mut self) -> T {
            self.0.take().unwrap().0
        }
    }
    impl ScopeGuard<(), fn(())> {
        pub fn new(cleanup: impl FnOnce()) -> ScopeGuard<(), impl FnOnce(())> {
            ScopeGuard::new_with_data((), move |()| cleanup())
        }
    }
    impl<T, F: FnOnce(T)> Deref for ScopeGuard<T, F> {
        type Target = T;
        fn deref(&self) -> &T {
            &self.0.as_ref().unwrap().0
        }
    }
    impl<T, F: FnOnce(T)> DerefMut for ScopeGuard<T, F> {
        fn deref_mut(&mut self) -> &mut T {
            &mut self.0.as_mut().unwrap().0
        }
    }
    impl<T, F: FnOnce(T)> Drop for ScopeGuard<T, F> {
        fn drop(&mut self) {
            if let Some((data, cleanup)) = self.0.take() {
                cleanup(data)
            }
        }
    }
    #[repr(transparent)]
    pub struct Opaque<T> {
        value: UnsafeCell<MaybeUninit<T>>,
        _pin: PhantomPinned,
    }
    impl<T> Opaque<T> {
        pub const fn new(value: T) -> Self {
            Self {
                value: UnsafeCell::new(MaybeUninit::new(value)),
                _pin: PhantomPinned,
            }
        }
        pub const fn uninit() -> Self {
            Self {
                value: UnsafeCell::new(MaybeUninit::uninit()),
                _pin: PhantomPinned,
            }
        }
        pub fn ffi_init(init_func: impl FnOnce(*mut T)) -> impl PinInit<Self> {
            unsafe {
                init::pin_init_from_closure::<_, ::core::convert::Infallible>(move |slot| {
                    init_func(Self::raw_get(slot));
                    Ok(())
                })
            }
        }
        pub fn try_ffi_init<E>(
            init_func: impl FnOnce(*mut T) -> Result<(), E>,
        ) -> impl PinInit<Self, E> {
            unsafe {
                init::pin_init_from_closure::<_, E>(move |slot| init_func(Self::raw_get(slot)))
            }
        }
        pub const fn get(&self) -> *mut T {
            UnsafeCell::get(&self.value).cast::<T>()
        }
        pub const fn raw_get(this: *const Self) -> *mut T {
            UnsafeCell::raw_get(this.cast::<UnsafeCell<MaybeUninit<T>>>()).cast::<T>()
        }
    }
    pub unsafe trait AlwaysRefCounted {
        fn inc_ref(&self);
        unsafe fn dec_ref(obj: NonNull<Self>);
    }
    pub struct ARef<T: AlwaysRefCounted> {
        ptr: NonNull<T>,
        _p: PhantomData<T>,
    }
    unsafe impl<T: AlwaysRefCounted + Sync + Send> Send for ARef<T> {}
    unsafe impl<T: AlwaysRefCounted + Sync + Send> Sync for ARef<T> {}
    impl<T: AlwaysRefCounted> ARef<T> {
        pub unsafe fn from_raw(ptr: NonNull<T>) -> Self {
            Self {
                ptr,
                _p: PhantomData,
            }
        }
        pub fn into_raw(me: Self) -> NonNull<T> {
            ManuallyDrop::new(me).ptr
        }
    }
    impl<T: AlwaysRefCounted> Clone for ARef<T> {
        fn clone(&self) -> Self {
            self.inc_ref();
            unsafe { Self::from_raw(self.ptr) }
        }
    }
    impl<T: AlwaysRefCounted> Deref for ARef<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            unsafe { self.ptr.as_ref() }
        }
    }
    impl<T: AlwaysRefCounted> From<&T> for ARef<T> {
        fn from(b: &T) -> Self {
            b.inc_ref();
            unsafe { Self::from_raw(NonNull::from(b)) }
        }
    }
    impl<T: AlwaysRefCounted> Drop for ARef<T> {
        fn drop(&mut self) {
            unsafe { T::dec_ref(self.ptr) };
        }
    }
    impl<T, U> PartialEq<ARef<U>> for ARef<T>
    where
        T: AlwaysRefCounted + PartialEq<U>,
        U: AlwaysRefCounted,
    {
        #[inline]
        fn eq(&self, other: &ARef<U>) -> bool {
            T::eq(&**self, &**other)
        }
    }
    impl<T: AlwaysRefCounted + Eq> Eq for ARef<T> {}
    impl<T, U> PartialEq<&'_ U> for ARef<T>
    where
        T: AlwaysRefCounted + PartialEq<U>,
    {
        #[inline]
        fn eq(&self, other: &&U) -> bool {
            T::eq(&**self, other)
        }
    }
    pub enum Either<L, R> {
        Left(L),
        Right(R),
    }
    pub unsafe trait FromBytes {}
    macro_rules! impl_frombytes {
    ($($({$($generics:tt)*})? $t:ty, )*) => {
        $(unsafe impl$($($generics)*)? FromBytes for $t {})*
    };
}
    impl_frombytes! {
        u8, u16, u32, u64, usize,
        i8, i16, i32, i64, isize,
        {<T: FromBytes>} [T],
        {<T: FromBytes, const N: usize>} [T; N],
        uapi::wrapfd_wrap,
        uapi::wrapfd_get_state,
        uapi::wrapfd_load,
        uapi::wrapfd_rewrap,
    }
    pub unsafe trait AsBytes {}
    pub type NotThreadSafe = PhantomData<*mut ()>;
    #[allow(non_upper_case_globals)]
    pub const NotThreadSafe: NotThreadSafe = PhantomData;
    macro_rules! impl_asbytes {
    ($($({$($generics:tt)*})? $t:ty, )*) => {
        $(unsafe impl$($($generics)*)? AsBytes for $t {})*
    };
}
    impl_asbytes! {
        u8, u16, u32, u64, usize,
        i8, i16, i32, i64, isize,
        bool,
        char,
        str,
        {<T: AsBytes>} [T],
        {<T: AsBytes, const N: usize>} [T; N],
        uapi::wrapfd_wrap,
        uapi::wrapfd_get_state,
        uapi::wrapfd_load,
        uapi::wrapfd_rewrap,
    }
}

pub mod uaccess {
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
        pub fn reader(self) -> UserSliceReader {
            UserSliceReader {
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
    }
    pub struct UserSliceWriter {
        ptr: UserPtr,
        length: usize,
    }
    impl UserSliceWriter {
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
}

pub mod alloc {
    #[cfg(not(any(test, testlib)))]
    pub mod allocator {
        use super::Flags;
        use crate::alloc::{AllocError, Allocator};
        use crate::bindings;
        use crate::pr_warn;
        use core::alloc::Layout;
        use core::ptr;
        use core::ptr::NonNull;
        pub struct Kmalloc;
        pub struct KVmalloc;
        struct ReallocFunc(
            unsafe extern "C" fn(
                *const crate::ffi::c_void,
                usize,
                usize,
                u32,
            ) -> *mut crate::ffi::c_void,
        );
        impl ReallocFunc {
            const KREALLOC: Self = Self(bindings::krealloc4);
            const KVREALLOC: Self = Self(bindings::kvrealloc);
            #[inline]
            unsafe fn call(
                &self,
                ptr: Option<NonNull<u8>>,
                layout: Layout,
                old_layout: Layout,
                flags: Flags,
            ) -> Result<NonNull<[u8]>, AllocError> {
                let size = layout.size();
                let ptr = match ptr {
                    Some(ptr) => {
                        if old_layout.size() == 0 {
                            ptr::null()
                        } else {
                            ptr.as_ptr()
                        }
                    }
                    None => ptr::null(),
                };
                let raw_ptr =
                    unsafe { self.0(ptr.cast(), old_layout.size(), size, flags.0).cast() };
                let ptr = if size == 0 {
                    crate::alloc::dangling_from_layout(layout)
                } else {
                    NonNull::new(raw_ptr).ok_or(AllocError)?
                };
                Ok(NonNull::slice_from_raw_parts(ptr, size))
            }
        }
        impl Kmalloc {
            pub fn aligned_layout(layout: Layout) -> Layout {
                layout.pad_to_align()
            }
        }
        unsafe impl Allocator for Kmalloc {
            #[inline]
            unsafe fn realloc(
                ptr: Option<NonNull<u8>>,
                layout: Layout,
                old_layout: Layout,
                flags: Flags,
            ) -> Result<NonNull<[u8]>, AllocError> {
                let layout = Kmalloc::aligned_layout(layout);
                unsafe { ReallocFunc::KREALLOC.call(ptr, layout, old_layout, flags) }
            }
        }
        unsafe impl Allocator for KVmalloc {
            #[inline]
            unsafe fn realloc(
                ptr: Option<NonNull<u8>>,
                layout: Layout,
                old_layout: Layout,
                flags: Flags,
            ) -> Result<NonNull<[u8]>, AllocError> {
                let layout = Kmalloc::aligned_layout(layout);
                if layout.align() > bindings::PAGE_SIZE {
                    pr_warn!("KVmalloc does not support alignments larger than PAGE_SIZE yet.\n");
                    return Err(AllocError);
                }
                unsafe { ReallocFunc::KVREALLOC.call(ptr, layout, old_layout, flags) }
            }
        }
    }
    pub mod kbox {
        #[allow(unused_imports)] // Used in doc comments.
        use super::{AllocError, Allocator, Flags};
        use crate::init::{InPlaceInit, InPlaceWrite, Init, PinInit};
        use crate::types::ForeignOwnable;
        use core::alloc::Layout;
        use core::fmt;
        use core::marker::PhantomData;
        use core::mem::ManuallyDrop;
        use core::mem::MaybeUninit;
        use core::ops::{Deref, DerefMut};
        use core::pin::Pin;
        use core::ptr::NonNull;
        use core::result::Result;
        #[repr(transparent)]
        pub struct Box<T: ?Sized, A: Allocator>(NonNull<T>, PhantomData<A>);
        impl<T, U, A> core::ops::CoerceUnsized<Box<U, A>> for Box<T, A>
        where
            T: ?Sized + core::marker::Unsize<U>,
            U: ?Sized,
            A: Allocator,
        {
        }
        impl<T, U, A> core::ops::DispatchFromDyn<Box<U, A>> for Box<T, A>
        where
            T: ?Sized + core::marker::Unsize<U>,
            U: ?Sized,
            A: Allocator,
        {
        }
        pub type KBox<T> = Box<T, super::allocator::Kmalloc>;
        unsafe impl<T, A> Send for Box<T, A>
        where
            T: Send + ?Sized,
            A: Allocator,
        {
        }
        unsafe impl<T, A> Sync for Box<T, A>
        where
            T: Sync + ?Sized,
            A: Allocator,
        {
        }
        impl<T, A> Box<T, A>
        where
            T: ?Sized,
            A: Allocator,
        {
            #[inline]
            pub const unsafe fn from_raw(raw: *mut T) -> Self {
                Self(unsafe { NonNull::new_unchecked(raw) }, PhantomData)
            }
            #[inline]
            pub fn into_raw(b: Self) -> *mut T {
                ManuallyDrop::new(b).0.as_ptr()
            }
            #[inline]
            pub fn leak<'a>(b: Self) -> &'a mut T {
                unsafe { &mut *Box::into_raw(b) }
            }
        }
        impl<T, A> Box<MaybeUninit<T>, A>
        where
            A: Allocator,
        {
            pub unsafe fn assume_init(self) -> Box<T, A> {
                let raw = Self::into_raw(self);
                unsafe { Box::from_raw(raw.cast()) }
            }
            pub fn write(mut self, value: T) -> Box<T, A> {
                (*self).write(value);
                unsafe { self.assume_init() }
            }
        }
        impl<T, A> Box<T, A>
        where
            A: Allocator,
        {
            pub fn new(x: T, flags: Flags) -> Result<Self, AllocError> {
                let b = Self::new_uninit(flags)?;
                Ok(Box::write(b, x))
            }
            pub fn new_uninit(flags: Flags) -> Result<Box<MaybeUninit<T>, A>, AllocError> {
                let layout = Layout::new::<MaybeUninit<T>>();
                let ptr = A::alloc(layout, flags)?;
                Ok(Box(ptr.cast(), PhantomData))
            }
            #[inline]
            pub fn pin(x: T, flags: Flags) -> Result<Pin<Box<T, A>>, AllocError>
            where
                A: 'static,
            {
                Ok(Self::new(x, flags)?.into())
            }
            fn forget_contents(this: Self) -> Box<MaybeUninit<T>, A> {
                let ptr = Self::into_raw(this);
                unsafe { Box::from_raw(ptr.cast()) }
            }
            pub fn drop_contents(this: Self) -> Box<MaybeUninit<T>, A> {
                let ptr = this.0.as_ptr();
                unsafe { core::ptr::drop_in_place(ptr) };
                Self::forget_contents(this)
            }
            pub fn into_inner(b: Self) -> T {
                let value = unsafe { core::ptr::read(&*b) };
                let _ = Self::forget_contents(b);
                value
            }
        }
        impl<T, A> From<Box<T, A>> for Pin<Box<T, A>>
        where
            T: ?Sized,
            A: Allocator,
        {
            fn from(b: Box<T, A>) -> Self {
                unsafe { Pin::new_unchecked(b) }
            }
        }
        impl<T, A> InPlaceWrite<T> for Box<MaybeUninit<T>, A>
        where
            A: Allocator + 'static,
        {
            type Initialized = Box<T, A>;
            fn write_init<E>(mut self, init: impl Init<T, E>) -> Result<Self::Initialized, E> {
                let slot = self.as_mut_ptr();
                unsafe { init.__init(slot)? };
                Ok(unsafe { Box::assume_init(self) })
            }
            fn write_pin_init<E>(
                mut self,
                init: impl PinInit<T, E>,
            ) -> Result<Pin<Self::Initialized>, E> {
                let slot = self.as_mut_ptr();
                unsafe { init.__pinned_init(slot)? };
                Ok(unsafe { Box::assume_init(self) }.into())
            }
        }
        impl<T, A> InPlaceInit<T> for Box<T, A>
        where
            A: Allocator + 'static,
        {
            type PinnedSelf = Pin<Self>;
            #[inline]
            fn try_pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> Result<Pin<Self>, E>
            where
                E: From<AllocError>,
            {
                Box::<_, A>::new_uninit(flags)?.write_pin_init(init)
            }
            #[inline]
            fn try_init<E>(init: impl Init<T, E>, flags: Flags) -> Result<Self, E>
            where
                E: From<AllocError>,
            {
                Box::<_, A>::new_uninit(flags)?.write_init(init)
            }
        }
        impl<T: 'static, A> ForeignOwnable for Box<T, A>
        where
            A: Allocator,
        {
            type Borrowed<'a> = &'a T;
            fn into_foreign(self) -> *const crate::ffi::c_void {
                Box::into_raw(self) as _
            }
            unsafe fn from_foreign(ptr: *const crate::ffi::c_void) -> Self {
                unsafe { Box::from_raw(ptr as _) }
            }
            unsafe fn borrow<'a>(ptr: *const crate::ffi::c_void) -> &'a T {
                unsafe { &*ptr.cast() }
            }
        }
        impl<T: 'static, A> ForeignOwnable for Pin<Box<T, A>>
        where
            A: Allocator,
        {
            type Borrowed<'a> = Pin<&'a T>;
            fn into_foreign(self) -> *const crate::ffi::c_void {
                Box::into_raw(unsafe { Pin::into_inner_unchecked(self) }) as _
            }
            unsafe fn from_foreign(ptr: *const crate::ffi::c_void) -> Self {
                unsafe { Pin::new_unchecked(Box::from_raw(ptr as _)) }
            }
            unsafe fn borrow<'a>(ptr: *const crate::ffi::c_void) -> Pin<&'a T> {
                let r = unsafe { &*ptr.cast() };
                unsafe { Pin::new_unchecked(r) }
            }
        }
        impl<T, A> Deref for Box<T, A>
        where
            T: ?Sized,
            A: Allocator,
        {
            type Target = T;
            fn deref(&self) -> &T {
                unsafe { self.0.as_ref() }
            }
        }
        impl<T, A> DerefMut for Box<T, A>
        where
            T: ?Sized,
            A: Allocator,
        {
            fn deref_mut(&mut self) -> &mut T {
                unsafe { self.0.as_mut() }
            }
        }
        impl<T, A> fmt::Debug for Box<T, A>
        where
            T: ?Sized + fmt::Debug,
            A: Allocator,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&**self, f)
            }
        }
        impl<T, A> Drop for Box<T, A>
        where
            T: ?Sized,
            A: Allocator,
        {
            fn drop(&mut self) {
                let layout = Layout::for_value::<T>(self);
                unsafe { core::ptr::drop_in_place::<T>(self.deref_mut()) };
                unsafe { A::free(self.0.cast(), layout) };
            }
        }
    }
    pub mod kvec {
        use super::{
            allocator::{KVmalloc, Kmalloc},
            layout::ArrayLayout,
            AllocError, Allocator, Flags,
        };
        use core::{
            marker::PhantomData,
            mem::{ManuallyDrop, MaybeUninit},
            ops::{Deref, DerefMut},
            ptr::{self, NonNull},
            slice,
        };

        pub struct Vec<T, A: Allocator> {
            ptr: NonNull<T>,
            layout: ArrayLayout<T>,
            len: usize,
            _p: PhantomData<A>,
        }

        unsafe impl<T: Send, A: Allocator> Send for Vec<T, A> {}
        unsafe impl<T: Sync, A: Allocator> Sync for Vec<T, A> {}

        pub type KVec<T> = Vec<T, Kmalloc>;
        pub type KVVec<T> = Vec<T, KVmalloc>;

        impl<T, A: Allocator> Vec<T, A> {
            #[inline]
            pub const fn is_zst() -> bool {
                core::mem::size_of::<T>() == 0
            }

            pub fn capacity(&self) -> usize {
                if Self::is_zst() {
                    usize::MAX
                } else {
                    self.layout.len()
                }
            }

            #[inline]
            pub fn len(&self) -> usize {
                self.len
            }

            #[inline]
            pub unsafe fn inc_len(&mut self, additional: usize) {
                self.len += additional;
            }

            #[inline]
            unsafe fn dec_len(&mut self, count: usize) -> &mut [T] {
                self.len -= count;
                unsafe { slice::from_raw_parts_mut(self.as_mut_ptr().add(self.len), count) }
            }

            #[inline]
            pub fn as_slice(&self) -> &[T] {
                self
            }

            #[inline]
            pub fn as_mut_slice(&mut self) -> &mut [T] {
                self
            }

            #[inline]
            pub fn as_mut_ptr(&mut self) -> *mut T {
                self.ptr.as_ptr()
            }

            #[inline]
            pub fn as_ptr(&self) -> *const T {
                self.ptr.as_ptr()
            }

            #[inline]
            pub fn is_empty(&self) -> bool {
                self.len == 0
            }

            pub const fn new() -> Self {
                Self {
                    ptr: NonNull::dangling(),
                    layout: ArrayLayout::empty(),
                    len: 0,
                    _p: PhantomData::<A>,
                }
            }

            pub fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<T>] {
                let ptr = unsafe { self.as_mut_ptr().add(self.len) } as *mut MaybeUninit<T>;
                unsafe { slice::from_raw_parts_mut(ptr, self.capacity() - self.len) }
            }

            pub fn push(&mut self, v: T, flags: Flags) -> Result<(), AllocError> {
                self.reserve(1, flags)?;
                let spare = self.spare_capacity_mut();
                unsafe { spare.get_unchecked_mut(0) }.write(v);
                unsafe { self.inc_len(1) };
                Ok(())
            }

            pub fn pop(&mut self) -> Option<T> {
                if self.is_empty() {
                    return None;
                }
                let removed: *mut T = {
                    let slice = unsafe { self.dec_len(1) };
                    unsafe { slice.get_unchecked_mut(0) }
                };
                Some(unsafe { removed.read() })
            }

            pub fn remove(&mut self, i: usize) -> Result<T, RemoveError> {
                let value = {
                    let value_ref = self.get(i).ok_or(RemoveError)?;
                    unsafe { ptr::read(value_ref) }
                };
                let p = unsafe { self.as_mut_ptr().add(i) };
                unsafe { ptr::copy(p.add(1), p, self.len - i - 1) };
                unsafe { self.dec_len(1) };
                Ok(value)
            }

            pub fn with_capacity(capacity: usize, flags: Flags) -> Result<Self, AllocError> {
                let mut v = Vec::new();
                v.reserve(capacity, flags)?;
                Ok(v)
            }

            pub unsafe fn from_raw_parts(ptr: *mut T, length: usize, capacity: usize) -> Self {
                let layout = if Self::is_zst() {
                    ArrayLayout::empty()
                } else {
                    unsafe { ArrayLayout::new_unchecked(capacity) }
                };
                Self {
                    ptr: unsafe { NonNull::new_unchecked(ptr) },
                    layout,
                    len: length,
                    _p: PhantomData,
                }
            }

            pub fn reserve(&mut self, additional: usize, flags: Flags) -> Result<(), AllocError> {
                let len = self.len();
                let cap = self.capacity();
                if cap - len >= additional {
                    return Ok(());
                }
                if Self::is_zst() {
                    return Err(AllocError);
                }
                let new_cap = core::cmp::max(
                    cap.checked_mul(2).ok_or(AllocError)?,
                    len.checked_add(additional).ok_or(AllocError)?,
                );
                let new_layout = ArrayLayout::new(new_cap).map_err(|_| AllocError)?;
                let ptr = if self.layout.len() == 0 {
                    None
                } else {
                    Some(self.ptr.cast())
                };
                let ptr = unsafe { A::realloc(ptr, new_layout.into(), self.layout.into(), flags)? };
                self.ptr = ptr.cast();
                self.layout = new_layout;
                Ok(())
            }

            pub fn retain(&mut self, mut f: impl FnMut(&mut T) -> bool) {
                let mut num_kept = 0;
                let mut next_to_check = 0;
                while next_to_check < self.len() {
                    if f(&mut self[next_to_check]) {
                        self.swap(num_kept, next_to_check);
                        num_kept += 1;
                    } else {
                        self.swap(num_kept, next_to_check);
                        unsafe { ptr::drop_in_place(&mut self[num_kept]) };
                    }
                    next_to_check += 1;
                }
                self.len = num_kept;
            }
        }

        impl<T, A: Allocator> Drop for Vec<T, A> {
            fn drop(&mut self) {
                unsafe { ptr::drop_in_place(self.as_mut_slice()) };
                if !Self::is_zst() && self.layout.len() != 0 {
                    unsafe { A::free(self.ptr.cast(), self.layout.into()) };
                }
            }
        }

        impl<T, A: Allocator> Default for Vec<T, A> {
            #[inline]
            fn default() -> Self {
                Self::new()
            }
        }

        impl<T, A: Allocator> Deref for Vec<T, A> {
            type Target = [T];
            #[inline]
            fn deref(&self) -> &[T] {
                unsafe { slice::from_raw_parts(self.as_ptr(), self.len) }
            }
        }

        impl<T, A: Allocator> DerefMut for Vec<T, A> {
            #[inline]
            fn deref_mut(&mut self) -> &mut [T] {
                unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
            }
        }

        impl<T, I: slice::SliceIndex<[T]>, A: Allocator> core::ops::Index<I> for Vec<T, A> {
            type Output = I::Output;
            #[inline]
            fn index(&self, index: I) -> &Self::Output {
                self.as_slice().index(index)
            }
        }

        impl<T, I: slice::SliceIndex<[T]>, A: Allocator> core::ops::IndexMut<I> for Vec<T, A> {
            #[inline]
            fn index_mut(&mut self, index: I) -> &mut Self::Output {
                self.as_mut_slice().index_mut(index)
            }
        }

        impl<'a, T, A: Allocator> IntoIterator for &'a Vec<T, A> {
            type Item = &'a T;
            type IntoIter = slice::Iter<'a, T>;
            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }

        pub struct IntoIter<T, A: Allocator> {
            ptr: *mut T,
            buf: NonNull<T>,
            len: usize,
            layout: ArrayLayout<T>,
            _p: PhantomData<A>,
        }

        impl<T, A: Allocator> IntoIter<T, A> {
            fn into_raw_fields(self) -> (*mut T, NonNull<T>, usize, ArrayLayout<T>) {
                let me = ManuallyDrop::new(self);
                (me.ptr, me.buf, me.len, me.layout)
            }
        }

        impl<T, A: Allocator> Iterator for IntoIter<T, A> {
            type Item = T;
            fn next(&mut self) -> Option<T> {
                if self.len == 0 {
                    return None;
                }
                let current = self.ptr;
                if !Vec::<T, A>::is_zst() {
                    unsafe { self.ptr = self.ptr.add(1) };
                }
                self.len -= 1;
                Some(unsafe { current.read() })
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.len, Some(self.len))
            }
        }

        impl<T, A: Allocator> Drop for IntoIter<T, A> {
            fn drop(&mut self) {
                let (ptr, buf, len, layout) = unsafe { ptr::read(self) }.into_raw_fields();
                unsafe { ptr::drop_in_place(slice::from_raw_parts_mut(ptr, len)) };
                if !Vec::<T, A>::is_zst() && layout.len() != 0 {
                    unsafe { A::free(buf.cast(), layout.into()) };
                }
            }
        }

        impl<T, A: Allocator> IntoIterator for Vec<T, A> {
            type Item = T;
            type IntoIter = IntoIter<T, A>;
            fn into_iter(self) -> Self::IntoIter {
                let me = ManuallyDrop::new(self);
                IntoIter {
                    ptr: me.ptr.as_ptr(),
                    buf: me.ptr,
                    len: me.len,
                    layout: me.layout,
                    _p: PhantomData::<A>,
                }
            }
        }

        pub struct RemoveError;
        impl From<RemoveError> for crate::error::Error {
            fn from(_: RemoveError) -> Self {
                crate::error::code::EINVAL
            }
        }

        impl<'a, T, A: Allocator> IntoIterator for &'a mut Vec<T, A> {
            type Item = &'a mut T;
            type IntoIter = slice::IterMut<'a, T>;
            fn into_iter(self) -> Self::IntoIter {
                self.iter_mut()
            }
        }
    }
    pub mod layout {
        use core::{alloc::Layout, marker::PhantomData};
        pub struct LayoutError;
        pub struct ArrayLayout<T> {
            len: usize,
            _phantom: PhantomData<fn() -> T>,
        }
        impl<T> Clone for ArrayLayout<T> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<T> Copy for ArrayLayout<T> {}
        const ISIZE_MAX: usize = isize::MAX as usize;
        impl<T> ArrayLayout<T> {
            pub const fn empty() -> Self {
                Self {
                    len: 0,
                    _phantom: PhantomData,
                }
            }
            pub const fn new(len: usize) -> Result<Self, LayoutError> {
                match len.checked_mul(core::mem::size_of::<T>()) {
                    Some(size) if size <= ISIZE_MAX => Ok(Self {
                        len,
                        _phantom: PhantomData,
                    }),
                    _ => Err(LayoutError),
                }
            }
            pub unsafe fn new_unchecked(len: usize) -> Self {
                Self {
                    len,
                    _phantom: PhantomData,
                }
            }
            pub const fn len(&self) -> usize {
                self.len
            }
            pub const fn is_empty(&self) -> bool {
                self.len == 0
            }
        }
        impl<T> From<ArrayLayout<T>> for Layout {
            fn from(value: ArrayLayout<T>) -> Self {
                let res = Layout::array::<T>(value.len);
                unsafe { res.unwrap_unchecked() }
            }
        }
    }
    #[cfg(any(test, testlib))]
    pub use self::allocator_test as allocator;
    pub use self::kbox::Box;
    pub use self::kbox::KBox;
    pub use self::kvec::KVVec;
    pub use self::kvec::KVec;
    pub use self::kvec::Vec;
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    pub struct AllocError;
    use core::{alloc::Layout, ptr::NonNull};
    #[derive(Clone, Copy, PartialEq)]
    pub struct Flags(u32);
    impl Flags {
        pub fn as_raw(self) -> u32 {
            self.0
        }
    }
    impl core::ops::BitOr for Flags {
        type Output = Self;
        fn bitor(self, rhs: Self) -> Self::Output {
            Self(self.0 | rhs.0)
        }
    }
    impl core::ops::BitAnd for Flags {
        type Output = Self;
        fn bitand(self, rhs: Self) -> Self::Output {
            Self(self.0 & rhs.0)
        }
    }
    impl core::ops::Not for Flags {
        type Output = Self;
        fn not(self) -> Self::Output {
            Self(!self.0)
        }
    }
    pub mod flags {
        use super::Flags;
        pub const __GFP_ZERO: Flags = Flags(bindings::__GFP_ZERO);
        pub const __GFP_HIGHMEM: Flags = Flags(bindings::__GFP_HIGHMEM);
        pub const GFP_KERNEL: Flags = Flags(bindings::GFP_KERNEL);
        pub const __GFP_NOWARN: Flags = Flags(bindings::__GFP_NOWARN);
    }
    pub unsafe trait Allocator {
        fn alloc(layout: Layout, flags: Flags) -> Result<NonNull<[u8]>, AllocError> {
            unsafe { Self::realloc(None, layout, Layout::new::<()>(), flags) }
        }
        unsafe fn realloc(
            ptr: Option<NonNull<u8>>,
            layout: Layout,
            old_layout: Layout,
            flags: Flags,
        ) -> Result<NonNull<[u8]>, AllocError>;
        unsafe fn free(ptr: NonNull<u8>, layout: Layout) {
            let _ = unsafe { Self::realloc(Some(ptr), Layout::new::<()>(), layout, Flags(0)) };
        }
    }
    pub fn dangling_from_layout(layout: Layout) -> NonNull<u8> {
        let ptr = layout.align() as *mut u8;
        unsafe { NonNull::new_unchecked(ptr) }
    }
}

pub mod dma_buf {
    pub mod buf {
        use crate::{
            bindings,
            dma::DataDirection,
            error::{from_err_ptr, to_result},
            fs::File,
            prelude::*,
            sync::SpinLock,
            types::{ARef, AlwaysRefCounted, Opaque},
        };
        use core::{
            marker::PhantomData,
            ops::{Bound, RangeBounds},
            ptr::{self, NonNull},
        };
        #[repr(transparent)]
        pub struct DmaBuf(Opaque<bindings::dma_buf>);
        unsafe impl Send for DmaBuf {}
        unsafe impl Sync for DmaBuf {}
        unsafe impl AlwaysRefCounted for DmaBuf {
            fn inc_ref(&self) {
                unsafe { bindings::get_dma_buf(self.as_ptr()) };
            }
            unsafe fn dec_ref(obj: NonNull<Self>) {
                unsafe { bindings::dma_buf_put(obj.as_ptr().cast()) };
            }
        }
        impl PartialEq for DmaBuf {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                ptr::eq(self.as_ptr(), other.as_ptr())
            }
        }
        impl Eq for DmaBuf {}
        impl DmaBuf {
            pub fn get(fd: i32) -> Result<ARef<DmaBuf>> {
                let ptr = unsafe { bindings::dma_buf_get(fd) };
                let ptr = from_err_ptr(ptr)?;
                let non_null = NonNull::new(ptr).ok_or(EBADF)?;
                Ok(unsafe { ARef::from_raw(non_null.cast()) })
            }
            #[inline]
            pub fn as_ptr(&self) -> *mut bindings::dma_buf {
                self.0.get()
            }
            pub fn vmap(&self) -> Result<DmaBufVmap> {
                let mut map = bindings::iosys_map::default();
                let ret = unsafe { bindings::dma_buf_vmap(self.as_ptr(), &raw mut map) };
                to_result(ret)?;
                let dmabuf = ARef::from(self);
                Ok(DmaBufVmap { dmabuf, map })
            }
            #[inline]
            pub fn size(&self) -> usize {
                unsafe { (*self.as_ptr()).size }
            }
            pub fn file(&self) -> &File {
                unsafe { File::from_raw_file((*self.as_ptr()).file) }
            }
            pub fn exporter_name(&self) -> &CStr {
                let exporter_name = unsafe { (*self.as_ptr()).exp_name };
                unsafe { CStr::from_char_ptr(exporter_name) }
            }
            pub fn with_name<T>(&self, f: impl FnOnce(Option<&CStr>) -> T) -> T {
                let name_lock = unsafe { &raw mut (*self.as_ptr()).name_lock };
                let _guard = unsafe { SpinLock::from_raw(name_lock) }.lock();
                let name = unsafe { (*self.as_ptr()).name };
                let opt_name = if name.is_null() {
                    None
                } else {
                    let name_as_cstr = unsafe { CStr::from_char_ptr(name) };
                    Some(name_as_cstr)
                };
                f(opt_name)
            }
        }
        pub struct DmaBufVmap {
            dmabuf: ARef<DmaBuf>,
            map: bindings::iosys_map,
        }
        unsafe impl Send for DmaBufVmap {}
        unsafe impl Sync for DmaBufVmap {}
        impl DmaBufVmap {
            #[inline]
            pub fn vaddr(&self) -> Option<NonNull<crate::ffi::c_void>> {
                if self.map.is_iomem {
                    None
                } else {
                    let vaddr = unsafe { self.map.__bindgen_anon_1.vaddr };
                    NonNull::new(vaddr)
                }
            }
            fn begin_cpu_access_fallible<const READ: bool, const WRITE: bool, F, T, E>(
                &self,
                f: F,
            ) -> Result<T, E>
            where
                F: FnOnce(CpuAccess<'_, READ, WRITE>) -> Result<T, E>,
                E: From<Error>,
            {
                match self.begin_cpu_access::<READ, WRITE, _, _>(f) {
                    Ok(res) => res,
                    Err(err) => Err(err.into()),
                }
            }
            fn begin_cpu_access<const READ: bool, const WRITE: bool, F, T>(&self, f: F) -> Result<T>
            where
                F: FnOnce(CpuAccess<'_, READ, WRITE>) -> T,
            {
                let dir = const {
                    match (READ, WRITE) {
                        (false, false) => DataDirection::None,
                        (false, true) => DataDirection::ToDevice,
                        (true, false) => DataDirection::FromDevice,
                        (true, true) => DataDirection::Bidirectional,
                    }
                };
                let mut map = self.map;
                to_result(unsafe {
                    bindings::dma_buf_begin_cpu_access(self.dmabuf.as_ptr(), dir.into())
                })?;
                let result = f(CpuAccess {
                    map: &raw mut map,
                    size: self.dmabuf.size(),
                    _lifetime: PhantomData,
                });
                to_result(unsafe {
                    bindings::dma_buf_end_cpu_access(self.dmabuf.as_ptr(), dir.into())
                })?;
                Ok(result)
            }
            pub fn begin_cpu_access_to<F, T>(&self, f: F) -> Result<T>
            where
                F: FnOnce(CpuAccess<'_, false, true>) -> T,
            {
                self.begin_cpu_access(f)
            }
            pub fn begin_cpu_access_bidirectional_fallible<F, T, E>(&self, f: F) -> Result<T, E>
            where
                F: FnOnce(CpuAccess<'_, true, true>) -> Result<T, E>,
                E: From<Error>,
            {
                self.begin_cpu_access_fallible(f)
            }
        }
        impl Drop for DmaBufVmap {
            #[inline]
            fn drop(&mut self) {
                unsafe { bindings::dma_buf_vunmap(self.dmabuf.as_ptr(), &mut self.map) };
            }
        }
        pub struct CpuAccess<'access, const READ: bool, const WRITE: bool> {
            map: *mut bindings::iosys_map,
            size: usize,
            _lifetime: PhantomData<&'access DmaBufVmap>,
        }
        impl<'a, const READ: bool> CpuAccess<'a, READ, true> {
            #[inline]
            #[track_caller]
            pub fn memcpy_to(&mut self, offset: usize, src: &[u8]) {
                let (end, end_overflow) = offset.overflowing_add(src.len());
                if end_overflow || end > self.size {
                    panic!(
                        "memcpy_to of {} bytes at {offset} in dma-buf of {} bytes out of bounds",
                        src.len(),
                        self.size
                    );
                }
                unsafe {
                    bindings::iosys_map_memcpy_to(self.map, offset, src.as_ptr().cast(), src.len())
                };
            }
            #[inline]
            #[track_caller]
            pub fn memset(&mut self, offset: usize, value: u8, len: usize) {
                let (end, end_overflow) = offset.overflowing_add(len);
                if end_overflow || end > self.size {
                    panic!("memset of {len} bytes at {offset} in dma-buf out of bounds",);
                }
                unsafe {
                    bindings::iosys_map_memset(self.map, offset, value as crate::ffi::c_int, len)
                };
            }
        }
        impl<'a, const WRITE: bool> CpuAccess<'a, true, WRITE> {}
        impl<'a> CpuAccess<'a, true, true> {
            #[inline]
            pub fn copy_within<R: RangeBounds<usize>>(&mut self, src: R, dst: usize) -> Result {
                let src_start = match src.start_bound() {
                    Bound::Included(&start) => start,
                    Bound::Excluded(&start) => start.checked_add(1).ok_or(EINVAL)?,
                    Bound::Unbounded => 0,
                };
                let src_end = match src.end_bound() {
                    Bound::Included(&end) => end.checked_add(1).ok_or(EINVAL)?,
                    Bound::Excluded(&end) => end,
                    Bound::Unbounded => self.size,
                };
                let len = src_end.checked_sub(src_start).ok_or(EINVAL)?;
                if src_end > self.size {
                    return Err(EINVAL);
                }
                let dst_end = dst.checked_add(len).ok_or(EINVAL)?;
                if dst_end > self.size {
                    return Err(EINVAL);
                }
                let map = unsafe { &*self.map };
                if map.is_iomem {
                    return Err(EINVAL);
                }
                let vaddr = unsafe { map.__bindgen_anon_1.vaddr }.cast::<u8>();
                if vaddr.is_null() {
                    return Err(EINVAL);
                }
                unsafe { ptr::copy(vaddr.add(src_start), vaddr.add(dst), len) };
                Ok(())
            }
        }
    }
    pub use buf::{CpuAccess, DmaBuf, DmaBufVmap};
}

pub mod fs {
    pub mod file {
        use crate::{
            bindings,
            error::{code::*, Error, Result},
            types::{ARef, AlwaysRefCounted, NotThreadSafe, Opaque},
        };
        use core::ptr;
        pub type Offset = bindings::loff_t;
        pub mod flags {}
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
            pub fn as_ptr(&self) -> *mut bindings::file {
                self.inner.get()
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
    }
    pub use self::file::{File, LocalFile};
}

pub mod init {
    use crate::{
        alloc::{AllocError, Flags, KBox},
        error::{self, Error},
        sync::Arc,
        sync::UniqueArc,
        types::{Opaque, ScopeGuard},
    };
    use core::{
        cell::UnsafeCell,
        convert::Infallible,
        marker::PhantomData,
        mem::MaybeUninit,
        num::*,
        pin::Pin,
        ptr::{self, NonNull},
    };
    #[doc(hidden)]
    pub mod __internal {
        use super::*;
        pub(super) type Invariant<T> = PhantomData<fn(*mut T) -> *mut T>;
        pub struct InitClosure<F, T: ?Sized, E>(pub F, pub Invariant<(E, T)>);
        unsafe impl<T: ?Sized, F, E> Init<T, E> for InitClosure<F, T, E>
        where
            F: FnOnce(*mut T) -> Result<(), E>,
        {
            #[inline]
            unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
                (self.0)(slot)
            }
        }
        unsafe impl<T: ?Sized, F, E> PinInit<T, E> for InitClosure<F, T, E>
        where
            F: FnOnce(*mut T) -> Result<(), E>,
        {
            #[inline]
            unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
                (self.0)(slot)
            }
        }
        pub unsafe trait HasPinData {
            type PinData: PinData;
            #[expect(clippy::missing_safety_doc)]
            unsafe fn __pin_data() -> Self::PinData;
        }
        pub unsafe trait PinData: Copy {
            type Datee: ?Sized + HasPinData;
            fn make_closure<F, O, E>(self, f: F) -> F
            where
                F: FnOnce(*mut Self::Datee) -> Result<O, E>,
            {
                f
            }
        }
        pub unsafe trait HasInitData {
            type InitData: InitData;
            #[expect(clippy::missing_safety_doc)]
            unsafe fn __init_data() -> Self::InitData;
        }
        pub unsafe trait InitData: Copy {
            type Datee: ?Sized + HasInitData;
            fn make_closure<F, O, E>(self, f: F) -> F
            where
                F: FnOnce(*mut Self::Datee) -> Result<O, E>,
            {
                f
            }
        }
        pub struct AllData<T: ?Sized>(PhantomData<fn(KBox<T>) -> KBox<T>>);
        impl<T: ?Sized> Clone for AllData<T> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<T: ?Sized> Copy for AllData<T> {}
        unsafe impl<T: ?Sized> InitData for AllData<T> {
            type Datee = T;
        }
        unsafe impl<T: ?Sized> HasInitData for T {
            type InitData = AllData<T>;
            unsafe fn __init_data() -> Self::InitData {
                AllData(PhantomData)
            }
        }
        pub struct StackInit<T> {
            value: MaybeUninit<T>,
            is_init: bool,
        }
        impl<T> Drop for StackInit<T> {
            #[inline]
            fn drop(&mut self) {
                if self.is_init {
                    unsafe { self.value.assume_init_drop() };
                }
            }
        }
        impl<T> StackInit<T> {
            #[inline]
            pub fn uninit() -> Self {
                Self {
                    value: MaybeUninit::uninit(),
                    is_init: false,
                }
            }
            #[inline]
            pub fn init<E>(
                self: Pin<&mut Self>,
                init: impl PinInit<T, E>,
            ) -> Result<Pin<&mut T>, E> {
                let this = unsafe { Pin::into_inner_unchecked(self) };
                if this.is_init {
                    this.is_init = false;
                    unsafe { this.value.assume_init_drop() };
                }
                unsafe { init.__pinned_init(this.value.as_mut_ptr())? };
                this.is_init = true;
                Ok(unsafe { Pin::new_unchecked(this.value.assume_init_mut()) })
            }
        }
        pub struct DropGuard<T: ?Sized> {
            ptr: *mut T,
        }
        impl<T: ?Sized> DropGuard<T> {
            #[inline]
            pub unsafe fn new(ptr: *mut T) -> Self {
                Self { ptr }
            }
            #[inline]
            pub fn let_binding(&mut self) -> &mut T {
                unsafe { &mut *self.ptr }
            }
        }
        impl<T: ?Sized> Drop for DropGuard<T> {
            #[inline]
            fn drop(&mut self) {
                unsafe { ptr::drop_in_place(self.ptr) }
            }
        }
        pub struct OnlyCallFromDrop(());
        impl OnlyCallFromDrop {
            pub unsafe fn new() -> Self {
                Self(())
            }
        }
        pub struct AlwaysFail<T: ?Sized> {
            _t: PhantomData<T>,
        }
        impl<T: ?Sized> AlwaysFail<T> {
            pub fn new() -> Self {
                Self { _t: PhantomData }
            }
        }
        impl<T: ?Sized> Default for AlwaysFail<T> {
            fn default() -> Self {
                Self::new()
            }
        }
        unsafe impl<T: ?Sized> PinInit<T, ()> for AlwaysFail<T> {
            unsafe fn __pinned_init(self, _slot: *mut T) -> Result<(), ()> {
                Err(())
            }
        }
    }
    #[doc(hidden)]
    pub mod macros {

        #[doc(hidden)]
        #[macro_export]
        macro_rules! __pinned_drop {
    (
        @impl_sig($($impl_sig:tt)*),
        @impl_body(
            $(#[$($attr:tt)*])*
            fn drop($($sig:tt)*) {
                $($inner:tt)*
            }
        ),
    ) => {
        unsafe $($impl_sig)* {
            $(#[$($attr)*])*
            fn drop($($sig)*, _: $crate::init::__internal::OnlyCallFromDrop) {
                $($inner)*
            }
        }
    }
}

        #[doc(hidden)]
        #[macro_export]
        macro_rules! __pin_data {
    (parse_input:
        @args($($pinned_drop:ident)?),
        @sig(
            $(#[$($struct_attr:tt)*])*
            $vis:vis struct $name:ident
            $(where $($whr:tt)*)?
        ),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @body({ $($fields:tt)* }),
    ) => {
        $crate::__pin_data!(find_pinned_fields:
            @struct_attrs($(#[$($struct_attr)*])*),
            @vis($vis),
            @name($name),
            @impl_generics($($impl_generics)*),
            @ty_generics($($ty_generics)*),
            @where($($($whr)*)?),
            @fields_munch($($fields)* ,),
            @pinned(),
            @not_pinned(),
            @fields(),
            @accum(),
            @is_pinned(),
            @pinned_drop($($pinned_drop)?),
        );
    };
    (find_pinned_fields:
        @struct_attrs($($struct_attrs:tt)*),
        @vis($vis:vis),
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @fields_munch($field:ident : $($($(::)?core::)?marker::)?PhantomPinned, $($rest:tt)*),
        @pinned($($pinned:tt)*),
        @not_pinned($($not_pinned:tt)*),
        @fields($($fields:tt)*),
        @accum($($accum:tt)*),
        @is_pinned(),
        @pinned_drop($($pinned_drop:ident)?),
    ) => {
        ::core::compile_error!(concat!(
            "The field `",
            stringify!($field),
            "` of type `PhantomPinned` only has an effect, if it has the `#[pin]` attribute.",
        ));
        $crate::__pin_data!(find_pinned_fields:
            @struct_attrs($($struct_attrs)*),
            @vis($vis),
            @name($name),
            @impl_generics($($impl_generics)*),
            @ty_generics($($ty_generics)*),
            @where($($whr)*),
            @fields_munch($($rest)*),
            @pinned($($pinned)* $($accum)* $field: ::core::marker::PhantomPinned,),
            @not_pinned($($not_pinned)*),
            @fields($($fields)* $($accum)* $field: ::core::marker::PhantomPinned,),
            @accum(),
            @is_pinned(),
            @pinned_drop($($pinned_drop)?),
        );
    };
    (find_pinned_fields:
        @struct_attrs($($struct_attrs:tt)*),
        @vis($vis:vis),
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @fields_munch($field:ident : $type:ty, $($rest:tt)*),
        @pinned($($pinned:tt)*),
        @not_pinned($($not_pinned:tt)*),
        @fields($($fields:tt)*),
        @accum($($accum:tt)*),
        @is_pinned(yes),
        @pinned_drop($($pinned_drop:ident)?),
    ) => {
        $crate::__pin_data!(find_pinned_fields:
            @struct_attrs($($struct_attrs)*),
            @vis($vis),
            @name($name),
            @impl_generics($($impl_generics)*),
            @ty_generics($($ty_generics)*),
            @where($($whr)*),
            @fields_munch($($rest)*),
            @pinned($($pinned)* $($accum)* $field: $type,),
            @not_pinned($($not_pinned)*),
            @fields($($fields)* $($accum)* $field: $type,),
            @accum(),
            @is_pinned(),
            @pinned_drop($($pinned_drop)?),
        );
    };
    (find_pinned_fields:
        @struct_attrs($($struct_attrs:tt)*),
        @vis($vis:vis),
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @fields_munch($field:ident : $type:ty, $($rest:tt)*),
        @pinned($($pinned:tt)*),
        @not_pinned($($not_pinned:tt)*),
        @fields($($fields:tt)*),
        @accum($($accum:tt)*),
        @is_pinned(),
        @pinned_drop($($pinned_drop:ident)?),
    ) => {
        $crate::__pin_data!(find_pinned_fields:
            @struct_attrs($($struct_attrs)*),
            @vis($vis),
            @name($name),
            @impl_generics($($impl_generics)*),
            @ty_generics($($ty_generics)*),
            @where($($whr)*),
            @fields_munch($($rest)*),
            @pinned($($pinned)*),
            @not_pinned($($not_pinned)* $($accum)* $field: $type,),
            @fields($($fields)* $($accum)* $field: $type,),
            @accum(),
            @is_pinned(),
            @pinned_drop($($pinned_drop)?),
        );
    };
    (find_pinned_fields:
        @struct_attrs($($struct_attrs:tt)*),
        @vis($vis:vis),
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @fields_munch(#[pin] $($rest:tt)*),
        @pinned($($pinned:tt)*),
        @not_pinned($($not_pinned:tt)*),
        @fields($($fields:tt)*),
        @accum($($accum:tt)*),
        @is_pinned($($is_pinned:ident)?),
        @pinned_drop($($pinned_drop:ident)?),
    ) => {
        $crate::__pin_data!(find_pinned_fields:
            @struct_attrs($($struct_attrs)*),
            @vis($vis),
            @name($name),
            @impl_generics($($impl_generics)*),
            @ty_generics($($ty_generics)*),
            @where($($whr)*),
            @fields_munch($($rest)*),
            @pinned($($pinned)*),
            @not_pinned($($not_pinned)*),
            @fields($($fields)*),
            @accum($($accum)*),
            @is_pinned(yes),
            @pinned_drop($($pinned_drop)?),
        );
    };
    (find_pinned_fields:
        @struct_attrs($($struct_attrs:tt)*),
        @vis($vis:vis),
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @fields_munch($fvis:vis $field:ident $($rest:tt)*),
        @pinned($($pinned:tt)*),
        @not_pinned($($not_pinned:tt)*),
        @fields($($fields:tt)*),
        @accum($($accum:tt)*),
        @is_pinned($($is_pinned:ident)?),
        @pinned_drop($($pinned_drop:ident)?),
    ) => {
        $crate::__pin_data!(find_pinned_fields:
            @struct_attrs($($struct_attrs)*),
            @vis($vis),
            @name($name),
            @impl_generics($($impl_generics)*),
            @ty_generics($($ty_generics)*),
            @where($($whr)*),
            @fields_munch($field $($rest)*),
            @pinned($($pinned)*),
            @not_pinned($($not_pinned)*),
            @fields($($fields)*),
            @accum($($accum)* $fvis),
            @is_pinned($($is_pinned)?),
            @pinned_drop($($pinned_drop)?),
        );
    };
    (find_pinned_fields:
        @struct_attrs($($struct_attrs:tt)*),
        @vis($vis:vis),
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @fields_munch(#[$($attr:tt)*] $($rest:tt)*),
        @pinned($($pinned:tt)*),
        @not_pinned($($not_pinned:tt)*),
        @fields($($fields:tt)*),
        @accum($($accum:tt)*),
        @is_pinned($($is_pinned:ident)?),
        @pinned_drop($($pinned_drop:ident)?),
    ) => {
        $crate::__pin_data!(find_pinned_fields:
            @struct_attrs($($struct_attrs)*),
            @vis($vis),
            @name($name),
            @impl_generics($($impl_generics)*),
            @ty_generics($($ty_generics)*),
            @where($($whr)*),
            @fields_munch($($rest)*),
            @pinned($($pinned)*),
            @not_pinned($($not_pinned)*),
            @fields($($fields)*),
            @accum($($accum)* #[$($attr)*]),
            @is_pinned($($is_pinned)?),
            @pinned_drop($($pinned_drop)?),
        );
    };
    (find_pinned_fields:
        @struct_attrs($($struct_attrs:tt)*),
        @vis($vis:vis),
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @fields_munch($(,)?),
        @pinned($($pinned:tt)*),
        @not_pinned($($not_pinned:tt)*),
        @fields($($fields:tt)*),
        @accum(),
        @is_pinned(),
        @pinned_drop($($pinned_drop:ident)?),
    ) => {
        $($struct_attrs)*
        $vis struct $name <$($impl_generics)*>
        where $($whr)*
        {
            $($fields)*
        }

        const _: () = {
            $vis struct __ThePinData<$($impl_generics)*>
            where $($whr)*
            {
                __phantom: ::core::marker::PhantomData<
                    fn($name<$($ty_generics)*>) -> $name<$($ty_generics)*>
                >,
            }

            impl<$($impl_generics)*> ::core::clone::Clone for __ThePinData<$($ty_generics)*>
            where $($whr)*
            {
                fn clone(&self) -> Self { *self }
            }

            impl<$($impl_generics)*> ::core::marker::Copy for __ThePinData<$($ty_generics)*>
            where $($whr)*
            {}

            $crate::__pin_data!(make_pin_data:
                @pin_data(__ThePinData),
                @impl_generics($($impl_generics)*),
                @ty_generics($($ty_generics)*),
                @where($($whr)*),
                @pinned($($pinned)*),
                @not_pinned($($not_pinned)*),
            );

            unsafe impl<$($impl_generics)*>
                $crate::init::__internal::HasPinData for $name<$($ty_generics)*>
            where $($whr)*
            {
                type PinData = __ThePinData<$($ty_generics)*>;

                unsafe fn __pin_data() -> Self::PinData {
                    __ThePinData { __phantom: ::core::marker::PhantomData }
                }
            }

            unsafe impl<$($impl_generics)*>
                $crate::init::__internal::PinData for __ThePinData<$($ty_generics)*>
            where $($whr)*
            {
                type Datee = $name<$($ty_generics)*>;
            }

            #[allow(dead_code)]
            struct __Unpin <'__pin, $($impl_generics)*>
            where $($whr)*
            {
                __phantom_pin: ::core::marker::PhantomData<fn(&'__pin ()) -> &'__pin ()>,
                __phantom: ::core::marker::PhantomData<
                    fn($name<$($ty_generics)*>) -> $name<$($ty_generics)*>
                >,
                $($pinned)*
            }

            #[doc(hidden)]
            impl<'__pin, $($impl_generics)*> ::core::marker::Unpin for $name<$($ty_generics)*>
            where
                __Unpin<'__pin, $($ty_generics)*>: ::core::marker::Unpin,
                $($whr)*
            {}

            $crate::__pin_data!(drop_prevention:
                @name($name),
                @impl_generics($($impl_generics)*),
                @ty_generics($($ty_generics)*),
                @where($($whr)*),
                @pinned_drop($($pinned_drop)?),
            );
        };
    };
    (drop_prevention:
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @pinned_drop(),
    ) => {
        trait MustNotImplDrop {}
        #[allow(drop_bounds)]
        impl<T: ::core::ops::Drop> MustNotImplDrop for T {}
        impl<$($impl_generics)*> MustNotImplDrop for $name<$($ty_generics)*>
        where $($whr)* {}
        #[allow(non_camel_case_types)]
        trait UselessPinnedDropImpl_you_need_to_specify_PinnedDrop {}
        impl<T: $crate::init::PinnedDrop>
            UselessPinnedDropImpl_you_need_to_specify_PinnedDrop for T {}
        impl<$($impl_generics)*>
            UselessPinnedDropImpl_you_need_to_specify_PinnedDrop for $name<$($ty_generics)*>
        where $($whr)* {}
    };
    (drop_prevention:
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @pinned_drop(PinnedDrop),
    ) => {
        impl<$($impl_generics)*> ::core::ops::Drop for $name<$($ty_generics)*>
        where $($whr)*
        {
            fn drop(&mut self) {
                let pinned = unsafe { ::core::pin::Pin::new_unchecked(self) };
                let token = unsafe { $crate::init::__internal::OnlyCallFromDrop::new() };
                $crate::init::PinnedDrop::drop(pinned, token);
            }
        }
    };
    (drop_prevention:
        @name($name:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @pinned_drop($($rest:tt)*),
    ) => {
        compile_error!(
            "Wrong parameters to `#[pin_data]`, expected nothing or `PinnedDrop`, got '{}'.",
            stringify!($($rest)*),
        );
    };
    (make_pin_data:
        @pin_data($pin_data:ident),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @where($($whr:tt)*),
        @pinned($($(#[$($p_attr:tt)*])* $pvis:vis $p_field:ident : $p_type:ty),* $(,)?),
        @not_pinned($($(#[$($attr:tt)*])* $fvis:vis $field:ident : $type:ty),* $(,)?),
    ) => {
        #[allow(dead_code)]
        impl<$($impl_generics)*> $pin_data<$($ty_generics)*>
        where $($whr)*
        {
            $(
                $(#[$($p_attr)*])*
                $pvis unsafe fn $p_field<E>(
                    self,
                    slot: *mut $p_type,
                    init: impl $crate::init::PinInit<$p_type, E>,
                ) -> ::core::result::Result<(), E> {
                    unsafe { $crate::init::PinInit::__pinned_init(init, slot) }
                }
            )*
            $(
                $(#[$($attr)*])*
                $fvis unsafe fn $field<E>(
                    self,
                    slot: *mut $type,
                    init: impl $crate::init::Init<$type, E>,
                ) -> ::core::result::Result<(), E> {
                    unsafe { $crate::init::Init::__init(init, slot) }
                }
            )*
        }
    };
}

        #[doc(hidden)]
        #[macro_export]
        macro_rules! __init_internal {
    (
        @this($($this:ident)?),
        @typ($t:path),
        @fields($($fields:tt)*),
        @error($err:ty),
        @data($data:ident, $($use_data:ident)?),
        @has_data($has_data:ident, $get_data:ident),
        @construct_closure($construct_closure:ident),
        @munch_fields(),
    ) => {
        $crate::__init_internal!(with_update_parsed:
            @this($($this)?),
            @typ($t),
            @fields($($fields)*),
            @error($err),
            @data($data, $($use_data)?),
            @has_data($has_data, $get_data),
            @construct_closure($construct_closure),
            @zeroed(), // Nothing means default behavior.
        )
    };
    (
        @this($($this:ident)?),
        @typ($t:path),
        @fields($($fields:tt)*),
        @error($err:ty),
        @data($data:ident, $($use_data:ident)?),
        @has_data($has_data:ident, $get_data:ident),
        @construct_closure($construct_closure:ident),
        @munch_fields(..Zeroable::zeroed()),
    ) => {
        $crate::__init_internal!(with_update_parsed:
            @this($($this)?),
            @typ($t),
            @fields($($fields)*),
            @error($err),
            @data($data, $($use_data)?),
            @has_data($has_data, $get_data),
            @construct_closure($construct_closure),
            @zeroed(()), // `()` means zero all fields not mentioned.
        )
    };
    (
        @this($($this:ident)?),
        @typ($t:path),
        @fields($($fields:tt)*),
        @error($err:ty),
        @data($data:ident, $($use_data:ident)?),
        @has_data($has_data:ident, $get_data:ident),
        @construct_closure($construct_closure:ident),
        @munch_fields($ignore:tt $($rest:tt)*),
    ) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t),
            @fields($($fields)*),
            @error($err),
            @data($data, $($use_data)?),
            @has_data($has_data, $get_data),
            @construct_closure($construct_closure),
            @munch_fields($($rest)*),
        )
    };
    (with_update_parsed:
        @this($($this:ident)?),
        @typ($t:path),
        @fields($($fields:tt)*),
        @error($err:ty),
        @data($data:ident, $($use_data:ident)?),
        @has_data($has_data:ident, $get_data:ident),
        @construct_closure($construct_closure:ident),
        @zeroed($($init_zeroed:expr)?),
    ) => {{
        struct __InitOk;
        let data = unsafe {
            use $crate::init::__internal::$has_data;
            ::kernel::macros::paste!($t::$get_data())
        };
        let init = $crate::init::__internal::$data::make_closure::<_, __InitOk, $err>(
            data,
            move |slot| {
                {
                    struct __InitOk;
                    $({
                        fn assert_zeroable<T: $crate::init::Zeroable>(_: *mut T) {}
                        assert_zeroable(slot);
                        unsafe { ::core::ptr::write_bytes(slot, 0, 1) };
                        $init_zeroed // This will be `()` if set.
                    })?
                    $(let $this = unsafe { ::core::ptr::NonNull::new_unchecked(slot) };)?
                    $crate::__init_internal!(init_slot($($use_data)?):
                        @data(data),
                        @slot(slot),
                        @guards(),
                        @munch_fields($($fields)*,),
                    );
                    #[allow(unreachable_code, clippy::diverging_sub_expression)]
                    let _ = || {
                        $crate::__init_internal!(make_initializer:
                            @slot(slot),
                            @type_name($t),
                            @munch_fields($($fields)*,),
                            @acc(),
                        );
                    };
                }
                Ok(__InitOk)
            }
        );
        let init = move |slot| -> ::core::result::Result<(), $err> {
            init(slot).map(|__InitOk| ())
        };
        let init = unsafe { $crate::init::$construct_closure::<_, $err>(init) };
        init
    }};
    (init_slot($($use_data:ident)?):
        @data($data:ident),
        @slot($slot:ident),
        @guards($($guards:ident,)*),
        @munch_fields($(..Zeroable::zeroed())? $(,)?),
    ) => {
        $(::core::mem::forget($guards);)*
    };
    (init_slot($use_data:ident): // `use_data` is present, so we use the `data` to init fields.
        @data($data:ident),
        @slot($slot:ident),
        @guards($($guards:ident,)*),
        @munch_fields($field:ident <- $val:expr, $($rest:tt)*),
    ) => {
        let init = $val;
        unsafe { $data.$field(::core::ptr::addr_of_mut!((*$slot).$field), init)? };
        ::kernel::macros::paste! {
            let [<$field>] = unsafe {
                $crate::init::__internal::DropGuard::new(::core::ptr::addr_of_mut!((*$slot).$field))
            };

            $crate::__init_internal!(init_slot($use_data):
                @data($data),
                @slot($slot),
                @guards([<$field>], $($guards,)*),
                @munch_fields($($rest)*),
            );
        }
    };
    (init_slot(): // No `use_data`, so we use `Init::__init` directly.
        @data($data:ident),
        @slot($slot:ident),
        @guards($($guards:ident,)*),
        @munch_fields($field:ident <- $val:expr, $($rest:tt)*),
    ) => {
        let init = $val;
        unsafe { $crate::init::Init::__init(init, ::core::ptr::addr_of_mut!((*$slot).$field))? };
        ::kernel::macros::paste! {
            let [<$field>] = unsafe {
                $crate::init::__internal::DropGuard::new(::core::ptr::addr_of_mut!((*$slot).$field))
            };

            $crate::__init_internal!(init_slot():
                @data($data),
                @slot($slot),
                @guards([<$field>], $($guards,)*),
                @munch_fields($($rest)*),
            );
        }
    };
    (init_slot($($use_data:ident)?):
        @data($data:ident),
        @slot($slot:ident),
        @guards($($guards:ident,)*),
        @munch_fields($field:ident $(: $val:expr)?, $($rest:tt)*),
    ) => {
        {
            $(let $field = $val;)?
            unsafe { ::core::ptr::write(::core::ptr::addr_of_mut!((*$slot).$field), $field) };
        }
        ::kernel::macros::paste! {
            let [<$field>] = unsafe {
                $crate::init::__internal::DropGuard::new(::core::ptr::addr_of_mut!((*$slot).$field))
            };

            $crate::__init_internal!(init_slot($($use_data)?):
                @data($data),
                @slot($slot),
                @guards([<$field>], $($guards,)*),
                @munch_fields($($rest)*),
            );
        }
    };
    (make_initializer:
        @slot($slot:ident),
        @type_name($t:path),
        @munch_fields(..Zeroable::zeroed() $(,)?),
        @acc($($acc:tt)*),
    ) => {
        #[allow(unused_assignments)]
        unsafe {
            let mut zeroed = ::core::mem::zeroed();
            ::core::ptr::write($slot, zeroed);
            zeroed = ::core::mem::zeroed();
            ::kernel::macros::paste!(
                ::core::ptr::write($slot, $t {
                    $($acc)*
                    ..zeroed
                });
            );
        }
    };
    (make_initializer:
        @slot($slot:ident),
        @type_name($t:path),
        @munch_fields($(,)?),
        @acc($($acc:tt)*),
    ) => {
        unsafe {
            ::kernel::macros::paste!(
                ::core::ptr::write($slot, $t {
                    $($acc)*
                });
            );
        }
    };
    (make_initializer:
        @slot($slot:ident),
        @type_name($t:path),
        @munch_fields($field:ident <- $val:expr, $($rest:tt)*),
        @acc($($acc:tt)*),
    ) => {
        $crate::__init_internal!(make_initializer:
            @slot($slot),
            @type_name($t),
            @munch_fields($($rest)*),
            @acc($($acc)* $field: ::core::panic!(),),
        );
    };
    (make_initializer:
        @slot($slot:ident),
        @type_name($t:path),
        @munch_fields($field:ident $(: $val:expr)?, $($rest:tt)*),
        @acc($($acc:tt)*),
    ) => {
        $crate::__init_internal!(make_initializer:
            @slot($slot),
            @type_name($t),
            @munch_fields($($rest)*),
            @acc($($acc)* $field: ::core::panic!(),),
        );
    };
}

        #[doc(hidden)]
        #[macro_export]
        macro_rules! __derive_zeroable {
    (parse_input:
        @sig(
            $(#[$($struct_attr:tt)*])*
            $vis:vis struct $name:ident
            $(where $($whr:tt)*)?
        ),
        @impl_generics($($impl_generics:tt)*),
        @ty_generics($($ty_generics:tt)*),
        @body({
            $(
                $(#[$($field_attr:tt)*])*
                $field:ident : $field_ty:ty
            ),* $(,)?
        }),
    ) => {
        #[automatically_derived]
        unsafe impl<$($impl_generics)*> $crate::init::Zeroable for $name<$($ty_generics)*>
        where
            $($($whr)*)?
        {}
        const _: () = {
            fn assert_zeroable<T: ?::core::marker::Sized + $crate::init::Zeroable>() {}
            fn ensure_zeroable<$($impl_generics)*>()
                where $($($whr)*)?
            {
                $(assert_zeroable::<$field_ty>();)*
            }
        };
    };
}
    }
    #[macro_export]
    macro_rules! pin_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error(::core::convert::Infallible),
            @data(PinData, use_data),
            @has_data(HasPinData, __pin_data),
            @construct_closure(pin_init_from_closure),
            @munch_fields($($fields)*),
        )
    };
}
    #[macro_export]
    macro_rules! try_pin_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)? ),
            @fields($($fields)*),
            @error($crate::error::Error),
            @data(PinData, use_data),
            @has_data(HasPinData, __pin_data),
            @construct_closure(pin_init_from_closure),
            @munch_fields($($fields)*),
        )
    };
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }? $err:ty) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)? ),
            @fields($($fields)*),
            @error($err),
            @data(PinData, use_data),
            @has_data(HasPinData, __pin_data),
            @construct_closure(pin_init_from_closure),
            @munch_fields($($fields)*),
        )
    };
}
    #[macro_export]
    macro_rules! init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error(::core::convert::Infallible),
            @data(InitData, /*no use_data*/),
            @has_data(HasInitData, __init_data),
            @construct_closure(init_from_closure),
            @munch_fields($($fields)*),
        )
    }
}
    #[macro_export]
    macro_rules! try_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error($crate::error::Error),
            @data(InitData, /*no use_data*/),
            @has_data(HasInitData, __init_data),
            @construct_closure(init_from_closure),
            @munch_fields($($fields)*),
        )
    };
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }? $err:ty) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error($err),
            @data(InitData, /*no use_data*/),
            @has_data(HasInitData, __init_data),
            @construct_closure(init_from_closure),
            @munch_fields($($fields)*),
        )
    };
}
    #[macro_export]
    macro_rules! assert_pinned {
        ($ty:ty, $field:ident, $field_ty:ty, inline) => {
            let _ = move |ptr: *mut $field_ty| {
                let data = unsafe { <$ty as $crate::init::__internal::HasPinData>::__pin_data() };
                let init = $crate::init::__internal::AlwaysFail::<$field_ty>::new();
                unsafe { data.$field(ptr, init) }.ok();
            };
        };
        ($ty:ty, $field:ident, $field_ty:ty) => {
            const _: () = {
                $crate::assert_pinned!($ty, $field, $field_ty, inline);
            };
        };
    }
    #[must_use = "An initializer must be used in order to create its value."]
    pub unsafe trait PinInit<T: ?Sized, E = Infallible>: Sized {
        unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E>;
        fn pin_chain<F>(self, f: F) -> ChainPinInit<Self, F, T, E>
        where
            F: FnOnce(Pin<&mut T>) -> Result<(), E>,
        {
            ChainPinInit(self, f, PhantomData)
        }
    }
    pub struct ChainPinInit<I, F, T: ?Sized, E>(I, F, __internal::Invariant<(E, KBox<T>)>);
    unsafe impl<T: ?Sized, E, I, F> PinInit<T, E> for ChainPinInit<I, F, T, E>
    where
        I: PinInit<T, E>,
        F: FnOnce(Pin<&mut T>) -> Result<(), E>,
    {
        unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
            unsafe { self.0.__pinned_init(slot)? };
            let val = unsafe { &mut *slot };
            let val = unsafe { Pin::new_unchecked(val) };
            (self.1)(val).inspect_err(|_| unsafe { core::ptr::drop_in_place(slot) })
        }
    }
    #[must_use = "An initializer must be used in order to create its value."]
    pub unsafe trait Init<T: ?Sized, E = Infallible>: PinInit<T, E> {
        unsafe fn __init(self, slot: *mut T) -> Result<(), E>;
        fn chain<F>(self, f: F) -> ChainInit<Self, F, T, E>
        where
            F: FnOnce(&mut T) -> Result<(), E>,
        {
            ChainInit(self, f, PhantomData)
        }
    }
    pub struct ChainInit<I, F, T: ?Sized, E>(I, F, __internal::Invariant<(E, KBox<T>)>);
    unsafe impl<T: ?Sized, E, I, F> Init<T, E> for ChainInit<I, F, T, E>
    where
        I: Init<T, E>,
        F: FnOnce(&mut T) -> Result<(), E>,
    {
        unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
            unsafe { self.0.__pinned_init(slot)? };
            (self.1)(unsafe { &mut *slot })
                .inspect_err(|_| unsafe { core::ptr::drop_in_place(slot) })
        }
    }
    unsafe impl<T: ?Sized, E, I, F> PinInit<T, E> for ChainInit<I, F, T, E>
    where
        I: Init<T, E>,
        F: FnOnce(&mut T) -> Result<(), E>,
    {
        unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
            unsafe { self.__init(slot) }
        }
    }
    #[inline]
    pub const unsafe fn pin_init_from_closure<T: ?Sized, E>(
        f: impl FnOnce(*mut T) -> Result<(), E>,
    ) -> impl PinInit<T, E> {
        __internal::InitClosure(f, PhantomData)
    }
    #[inline]
    pub const unsafe fn init_from_closure<T: ?Sized, E>(
        f: impl FnOnce(*mut T) -> Result<(), E>,
    ) -> impl Init<T, E> {
        __internal::InitClosure(f, PhantomData)
    }
    #[inline]
    pub fn uninit<T, E>() -> impl Init<MaybeUninit<T>, E> {
        unsafe { init_from_closure(|_| Ok(())) }
    }
    pub fn init_array_from_fn<I, const N: usize, T, E>(
        mut make_init: impl FnMut(usize) -> I,
    ) -> impl Init<[T; N], E>
    where
        I: Init<T, E>,
    {
        let init = move |slot: *mut [T; N]| {
            let slot = slot.cast::<T>();
            let mut init_count = ScopeGuard::new_with_data(0, |i| {
                unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(slot, i)) };
            });
            for i in 0..N {
                let init = make_init(i);
                let ptr = unsafe { slot.add(i) };
                unsafe { init.__init(ptr) }?;
                *init_count += 1;
            }
            init_count.dismiss();
            Ok(())
        };
        unsafe { init_from_closure(init) }
    }
    pub fn pin_init_array_from_fn<I, const N: usize, T, E>(
        mut make_init: impl FnMut(usize) -> I,
    ) -> impl PinInit<[T; N], E>
    where
        I: PinInit<T, E>,
    {
        let init = move |slot: *mut [T; N]| {
            let slot = slot.cast::<T>();
            let mut init_count = ScopeGuard::new_with_data(0, |i| {
                unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(slot, i)) };
            });
            for i in 0..N {
                let init = make_init(i);
                let ptr = unsafe { slot.add(i) };
                unsafe { init.__pinned_init(ptr) }?;
                *init_count += 1;
            }
            init_count.dismiss();
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }
    unsafe impl<T, E> Init<T, E> for T {
        unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
            unsafe { slot.write(self) };
            Ok(())
        }
    }
    unsafe impl<T, E> PinInit<T, E> for T {
        unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
            unsafe { self.__init(slot) }
        }
    }
    unsafe impl<T, E> Init<T, E> for Result<T, E> {
        unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
            unsafe { slot.write(self?) };
            Ok(())
        }
    }
    unsafe impl<T, E> PinInit<T, E> for Result<T, E> {
        unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
            unsafe { slot.write(self?) };
            Ok(())
        }
    }
    pub trait InPlaceInit<T>: Sized {
        type PinnedSelf;
        fn try_pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> Result<Self::PinnedSelf, E>
        where
            E: From<AllocError>;
        fn pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> error::Result<Self::PinnedSelf>
        where
            Error: From<E>,
        {
            let init = unsafe {
                pin_init_from_closure(|slot| init.__pinned_init(slot).map_err(|e| Error::from(e)))
            };
            Self::try_pin_init(init, flags)
        }
        fn try_init<E>(init: impl Init<T, E>, flags: Flags) -> Result<Self, E>
        where
            E: From<AllocError>;
        fn init<E>(init: impl Init<T, E>, flags: Flags) -> error::Result<Self>
        where
            Error: From<E>,
        {
            let init = unsafe {
                init_from_closure(|slot| init.__pinned_init(slot).map_err(|e| Error::from(e)))
            };
            Self::try_init(init, flags)
        }
    }
    impl<T> InPlaceInit<T> for Arc<T> {
        type PinnedSelf = Self;
        #[inline]
        fn try_pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> Result<Self::PinnedSelf, E>
        where
            E: From<AllocError>,
        {
            UniqueArc::try_pin_init(init, flags).map(|u| u.into())
        }
        #[inline]
        fn try_init<E>(init: impl Init<T, E>, flags: Flags) -> Result<Self, E>
        where
            E: From<AllocError>,
        {
            UniqueArc::try_init(init, flags).map(|u| u.into())
        }
    }
    impl<T> InPlaceInit<T> for UniqueArc<T> {
        type PinnedSelf = Pin<Self>;
        #[inline]
        fn try_pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> Result<Self::PinnedSelf, E>
        where
            E: From<AllocError>,
        {
            UniqueArc::new_uninit(flags)?.write_pin_init(init)
        }
        #[inline]
        fn try_init<E>(init: impl Init<T, E>, flags: Flags) -> Result<Self, E>
        where
            E: From<AllocError>,
        {
            UniqueArc::new_uninit(flags)?.write_init(init)
        }
    }
    pub trait InPlaceWrite<T> {
        type Initialized;
        fn write_init<E>(self, init: impl Init<T, E>) -> Result<Self::Initialized, E>;
        fn write_pin_init<E>(self, init: impl PinInit<T, E>) -> Result<Pin<Self::Initialized>, E>;
    }
    impl<T> InPlaceWrite<T> for UniqueArc<MaybeUninit<T>> {
        type Initialized = UniqueArc<T>;
        fn write_init<E>(mut self, init: impl Init<T, E>) -> Result<Self::Initialized, E> {
            let slot = self.as_mut_ptr();
            unsafe { init.__init(slot)? };
            Ok(unsafe { self.assume_init() })
        }
        fn write_pin_init<E>(
            mut self,
            init: impl PinInit<T, E>,
        ) -> Result<Pin<Self::Initialized>, E> {
            let slot = self.as_mut_ptr();
            unsafe { init.__pinned_init(slot)? };
            Ok(unsafe { self.assume_init() }.into())
        }
    }
    pub unsafe trait PinnedDrop: __internal::HasPinData {
        fn drop(self: Pin<&mut Self>, only_call_from_drop: __internal::OnlyCallFromDrop);
    }
}

pub mod mm {
    pub mod virt {
        use crate::{
            bindings,
            error::{code::EINVAL, to_result, Result},
            page::Page,
            types::Opaque,
        };
        use core::ops::Deref;
        #[repr(transparent)]
        pub struct VmaRef {
            vma: Opaque<bindings::vm_area_struct>,
        }
        impl VmaRef {
            #[inline]
            pub fn as_ptr(&self) -> *mut bindings::vm_area_struct {
                self.vma.get()
            }
            #[inline]
            pub fn flags(&self) -> vm_flags_t {
                unsafe { (*self.as_ptr()).__bindgen_anon_2.vm_flags }
            }
        }
        #[repr(transparent)]
        pub struct VmaNew {
            vma: VmaRef,
        }
        impl Deref for VmaNew {
            type Target = VmaRef;
            #[inline]
            fn deref(&self) -> &VmaRef {
                &self.vma
            }
        }
        impl VmaNew {
            #[inline]
            pub unsafe fn from_raw<'a>(vma: *mut bindings::vm_area_struct) -> &'a Self {
                unsafe { &*vma.cast() }
            }
            #[inline]
            unsafe fn update_flags(&self, set: vm_flags_t, unset: vm_flags_t) {
                let mut flags = self.flags();
                flags |= set;
                flags &= !unset;
                unsafe { (*self.as_ptr()).__bindgen_anon_2.__vm_flags = flags };
            }
            #[inline]
            pub fn set_dontcopy(&self) {
                unsafe { self.update_flags(flags::DONTCOPY, 0) };
            }
            #[inline]
            pub fn writable(&self) -> bool {
                (self.flags() & flags::WRITE) != 0
            }
            #[inline]
            pub fn try_clear_maywrite(&self) -> Result {
                if self.writable() {
                    return Err(EINVAL);
                }
                unsafe { self.update_flags(0, flags::MAYWRITE) };
                Ok(())
            }
            #[cfg(CONFIG_64BIT)]
            #[inline]
            pub fn set_no_remap_file_pages(&self) {
                unsafe { self.update_flags(flags::NO_REMAP_FILE_PAGES, 0) };
            }
        }
        #[doc(inline)]
        pub use bindings::vm_flags_t;
        pub mod flags {
            use super::vm_flags_t;
            use crate::bindings;
            pub const WRITE: vm_flags_t = bindings::VM_WRITE as _;
            pub const MAYWRITE: vm_flags_t = bindings::VM_MAYWRITE as _;
            pub const DONTCOPY: vm_flags_t = bindings::VM_DONTCOPY as _;
            #[cfg(CONFIG_64BIT)]
            pub const NO_REMAP_FILE_PAGES: vm_flags_t = bindings::VM_NO_REMAP_FILE_PAGES as _;
        }
    }
}

pub mod str {
    use core::fmt::{self, Write};
    use core::ops::Deref;

    pub mod parse_int {
        use crate::error::{code::*, Result};
        use crate::str::BStr;

        pub trait ParseInt: Sized {
            fn from_str(src: &BStr) -> Result<Self>;
        }

        impl ParseInt for usize {
            fn from_str(src: &BStr) -> Result<Self> {
                let s = core::str::from_utf8(src).map_err(|_| EINVAL)?;
                let (radix, digits) =
                    if let Some(n) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                        (16, n)
                    } else if let Some(n) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
                        (8, n)
                    } else if let Some(n) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
                        (2, n)
                    } else if s.starts_with('0') && s.len() > 1 {
                        (8, &s[1..])
                    } else {
                        (10, s)
                    };
                usize::from_str_radix(digits, radix).map_err(|_| EINVAL)
            }
        }
    }

    #[repr(transparent)]
    pub struct BStr([u8]);

    impl BStr {
        #[inline]
        pub const fn from_bytes(bytes: &[u8]) -> &Self {
            unsafe { &*(bytes as *const [u8] as *const BStr) }
        }
    }

    impl Deref for BStr {
        type Target = [u8];
        #[inline]
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl AsRef<BStr> for BStr {
        #[inline]
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

    #[repr(transparent)]
    pub struct CStr([u8]);

    impl CStr {
        #[inline]
        pub unsafe fn from_char_ptr<'a>(ptr: *const crate::ffi::c_char) -> &'a Self {
            let len = unsafe { crate::bindings::strlen(ptr) } + 1;
            let bytes = unsafe { core::slice::from_raw_parts(ptr.cast(), len) };
            unsafe { Self::from_bytes_with_nul_unchecked(bytes) }
        }

        #[inline]
        pub const unsafe fn from_bytes_with_nul_unchecked(bytes: &[u8]) -> &CStr {
            unsafe { &*(bytes as *const [u8] as *const CStr) }
        }

        #[inline]
        pub const fn as_char_ptr(&self) -> *const crate::ffi::c_char {
            self.0.as_ptr().cast()
        }

        #[inline]
        pub fn as_bytes(&self) -> &[u8] {
            &self.0[..self.0.len() - 1]
        }

        #[inline]
        pub const fn as_bytes_with_nul(&self) -> &[u8] {
            &self.0
        }
    }

    impl fmt::Display for CStr {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for &c in self.as_bytes() {
                f.write_char(c as char)?;
            }
            Ok(())
        }
    }

    impl Deref for CStr {
        type Target = BStr;
        #[inline]
        fn deref(&self) -> &Self::Target {
            BStr::from_bytes(self.as_bytes())
        }
    }

    #[macro_export]
    macro_rules! c_str {
        ($str:expr) => {{
            const S: &str = concat!($str, "\0");
            const C: &$crate::str::CStr =
                unsafe { $crate::str::CStr::from_bytes_with_nul_unchecked(S.as_bytes()) };
            C
        }};
    }

    pub struct RawFormatter {
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
        pub unsafe fn from_ptrs(pos: *mut u8, end: *mut u8) -> Self {
            Self {
                beg: pos as _,
                pos: pos as _,
                end: end as _,
            }
        }
        pub unsafe fn from_buffer(buf: *mut u8, len: usize) -> Self {
            let pos = buf as usize;
            Self {
                pos,
                beg: pos,
                end: pos.saturating_add(len),
            }
        }
        pub fn pos(&self) -> *mut u8 {
            self.pos as _
        }
        pub fn bytes_written(&self) -> usize {
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
}

pub mod sync {
    use crate::types::Opaque;
    pub mod arc {
        use crate::{
            alloc::{AllocError, Flags, KBox},
            bindings,
            init::{self, InPlaceInit, Init, PinInit},
            try_init,
            types::{ForeignOwnable, Opaque},
        };
        use core::{
            alloc::Layout,
            fmt,
            marker::{PhantomData, Unsize},
            mem::{ManuallyDrop, MaybeUninit},
            ops::{Deref, DerefMut},
            pin::Pin,
            ptr::NonNull,
        };
        use macros::pin_data;
        mod std_vendor {

            use crate::sync::{arc::ArcInner, Arc};
            use core::any::Any;

            impl Arc<dyn Any + Send + Sync> {
                pub fn downcast<T>(self) -> core::result::Result<Arc<T>, Self>
                where
                    T: Any + Send + Sync,
                {
                    if (*self).is::<T>() {
                        unsafe {
                            let ptr = self.ptr.cast::<ArcInner<T>>();
                            core::mem::forget(self);
                            Ok(Arc::from_inner(ptr))
                        }
                    } else {
                        Err(self)
                    }
                }
            }
        }
        pub struct Arc<T: ?Sized> {
            ptr: NonNull<ArcInner<T>>,
            _p: PhantomData<ArcInner<T>>,
        }
        #[pin_data]
        #[repr(C)]
        struct ArcInner<T: ?Sized> {
            refcount: Opaque<bindings::refcount_t>,
            data: T,
        }
        impl<T: ?Sized> ArcInner<T> {}
        impl<T: ?Sized + Unsize<U>, U: ?Sized> core::ops::CoerceUnsized<Arc<U>> for Arc<T> {}
        impl<T: ?Sized + Unsize<U>, U: ?Sized> core::ops::DispatchFromDyn<Arc<U>> for Arc<T> {}
        unsafe impl<T: ?Sized + Sync + Send> Send for Arc<T> {}
        unsafe impl<T: ?Sized + Sync + Send> Sync for Arc<T> {}
        impl<T> Arc<T> {}
        impl<T: ?Sized> Arc<T> {
            unsafe fn from_inner(inner: NonNull<ArcInner<T>>) -> Self {
                Arc {
                    ptr: inner,
                    _p: PhantomData,
                }
            }
        }
        impl<T: 'static> ForeignOwnable for Arc<T> {
            type Borrowed<'a> = ArcBorrow<'a, T>;
            fn into_foreign(self) -> *const crate::ffi::c_void {
                ManuallyDrop::new(self).ptr.as_ptr() as _
            }
            unsafe fn borrow<'a>(ptr: *const crate::ffi::c_void) -> ArcBorrow<'a, T> {
                let inner = NonNull::new(ptr as *mut ArcInner<T>).unwrap();
                unsafe { ArcBorrow::new(inner) }
            }
            unsafe fn from_foreign(ptr: *const crate::ffi::c_void) -> Self {
                unsafe { Self::from_inner(NonNull::new(ptr as _).unwrap()) }
            }
        }
        impl<T: ?Sized> Deref for Arc<T> {
            type Target = T;
            fn deref(&self) -> &Self::Target {
                unsafe { &self.ptr.as_ref().data }
            }
        }
        impl<T: ?Sized> AsRef<T> for Arc<T> {
            fn as_ref(&self) -> &T {
                self.deref()
            }
        }
        impl<T: ?Sized> Clone for Arc<T> {
            fn clone(&self) -> Self {
                unsafe { bindings::refcount_inc(self.ptr.as_ref().refcount.get()) };
                unsafe { Self::from_inner(self.ptr) }
            }
        }
        impl<T: ?Sized> Drop for Arc<T> {
            fn drop(&mut self) {
                let refcount = unsafe { self.ptr.as_ref() }.refcount.get();
                let is_zero = unsafe { bindings::refcount_dec_and_test(refcount) };
                if is_zero {
                    unsafe { drop(KBox::from_raw(self.ptr.as_ptr())) };
                }
            }
        }
        impl<T: ?Sized> From<UniqueArc<T>> for Arc<T> {
            fn from(item: UniqueArc<T>) -> Self {
                item.inner
            }
        }
        impl<T: ?Sized> From<Pin<UniqueArc<T>>> for Arc<T> {
            fn from(item: Pin<UniqueArc<T>>) -> Self {
                unsafe { Pin::into_inner_unchecked(item).inner }
            }
        }
        pub struct ArcBorrow<'a, T: ?Sized + 'a> {
            inner: NonNull<ArcInner<T>>,
            _p: PhantomData<&'a ()>,
        }
        impl<T: ?Sized + Unsize<U>, U: ?Sized> core::ops::DispatchFromDyn<ArcBorrow<'_, U>>
            for ArcBorrow<'_, T>
        {
        }
        impl<T: ?Sized> Clone for ArcBorrow<'_, T> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<T: ?Sized> Copy for ArcBorrow<'_, T> {}
        impl<T: ?Sized> ArcBorrow<'_, T> {
            unsafe fn new(inner: NonNull<ArcInner<T>>) -> Self {
                Self {
                    inner,
                    _p: PhantomData,
                }
            }
        }
        impl<T: ?Sized> From<ArcBorrow<'_, T>> for Arc<T> {
            fn from(b: ArcBorrow<'_, T>) -> Self {
                ManuallyDrop::new(unsafe { Arc::from_inner(b.inner) })
                    .deref()
                    .clone()
            }
        }
        impl<T: ?Sized> Deref for ArcBorrow<'_, T> {
            type Target = T;
            fn deref(&self) -> &Self::Target {
                unsafe { &self.inner.as_ref().data }
            }
        }
        pub struct UniqueArc<T: ?Sized> {
            inner: Arc<T>,
        }
        impl<T> UniqueArc<T> {
            pub fn new_uninit(flags: Flags) -> Result<UniqueArc<MaybeUninit<T>>, AllocError> {
                let inner = KBox::try_init::<AllocError>(
                    try_init!(ArcInner {
                refcount: Opaque::new(unsafe { bindings::REFCOUNT_INIT(1) }),
                data <- init::uninit::<T, AllocError>(),
            }? AllocError),
                    flags,
                )?;
                Ok(UniqueArc {
                    inner: unsafe { Arc::from_inner(KBox::leak(inner).into()) },
                })
            }
        }
        impl<T> UniqueArc<MaybeUninit<T>> {
            pub unsafe fn assume_init(self) -> UniqueArc<T> {
                let inner = ManuallyDrop::new(self).inner.ptr;
                UniqueArc {
                    inner: unsafe { Arc::from_inner(inner.cast()) },
                }
            }
        }
        impl<T: ?Sized> From<UniqueArc<T>> for Pin<UniqueArc<T>> {
            fn from(obj: UniqueArc<T>) -> Self {
                unsafe { Pin::new_unchecked(obj) }
            }
        }
        impl<T: ?Sized> Deref for UniqueArc<T> {
            type Target = T;
            fn deref(&self) -> &Self::Target {
                self.inner.deref()
            }
        }
        impl<T: ?Sized> DerefMut for UniqueArc<T> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut self.inner.ptr.as_mut().data }
            }
        }
        impl<T: fmt::Display + ?Sized> fmt::Display for UniqueArc<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self.deref(), f)
            }
        }
        impl<T: fmt::Display + ?Sized> fmt::Display for Arc<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self.deref(), f)
            }
        }
        impl<T: fmt::Debug + ?Sized> fmt::Debug for UniqueArc<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self.deref(), f)
            }
        }
        impl<T: fmt::Debug + ?Sized> fmt::Debug for Arc<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self.deref(), f)
            }
        }
    }
    pub mod completion {
        use crate::{bindings, prelude::*, types::Opaque};
        #[pin_data]
        pub struct Completion {
            #[pin]
            inner: Opaque<bindings::completion>,
        }
        unsafe impl Send for Completion {}
        unsafe impl Sync for Completion {}
        impl Completion {
            pub fn new() -> impl PinInit<Self> {
                pin_init!(Self {
                    inner <- Opaque::ffi_init(|slot: *mut bindings::completion| {
                        unsafe { bindings::init_completion(slot) };
                    }),
                })
            }
            fn as_raw(&self) -> *mut bindings::completion {
                self.inner.get()
            }
            pub fn complete_all(&self) {
                unsafe { bindings::complete_all(self.as_raw()) };
            }
            pub fn wait_for_completion(&self) {
                unsafe { bindings::wait_for_completion(self.as_raw()) };
            }
        }
    }
    pub mod lock {

        use super::LockClassKey;
        use crate::{
            bindings, init::PinInit, pin_init, str::CStr, types::Opaque, types::ScopeGuard,
        };
        use core::{cell::UnsafeCell, marker::PhantomData, marker::PhantomPinned};
        use macros::pin_data;

        pub mod mutex {

            use crate::bindings;

            #[macro_export]
            macro_rules! new_mutex {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::Mutex::new(
            $inner, $crate::optional_name!($($name)?), $crate::static_lock_class!())
    };
}

            pub type Mutex<T> = super::Lock<T, MutexBackend>;

            pub struct MutexBackend;

            unsafe impl super::Backend for MutexBackend {
                type State = bindings::mutex;
                type GuardState = ();

                unsafe fn init(
                    ptr: *mut Self::State,
                    name: *const core::ffi::c_char,
                    key: *mut bindings::lock_class_key,
                ) {
                    unsafe { bindings::__mutex_init(ptr, name, key) }
                }

                unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
                    unsafe { bindings::mutex_lock(ptr) };
                }

                unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
                    unsafe { bindings::mutex_unlock(ptr) };
                }
            }
        }
        pub mod spinlock {
            use crate::{bindings, ffi::c_char, str::CStr, sync::LockClassKey};

            #[macro_export]
            macro_rules! new_spinlock {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::SpinLock::new(
            $inner, $crate::optional_name!($($name)?), $crate::static_lock_class!())
    };
}
            pub use new_spinlock;

            pub type SpinLock<T> = super::Lock<T, SpinLockBackend>;

            pub struct SpinLockBackend;

            unsafe impl super::Backend for SpinLockBackend {
                type State = bindings::spinlock_t;
                type GuardState = ();

                #[inline]
                unsafe fn init(
                    ptr: *mut Self::State,
                    name: *const c_char,
                    key: *mut bindings::lock_class_key,
                ) {
                    unsafe { bindings::__spin_lock_init(ptr, name, key) }
                }

                #[inline]
                unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
                    unsafe { bindings::spin_lock(ptr) }
                }

                #[inline]
                unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
                    unsafe { bindings::spin_unlock(ptr) }
                }
            }

            #[macro_export]
            macro_rules! new_spinlock_irq {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::SpinLockIrq::new(
            $inner, $crate::optional_name!($($name)?), $crate::static_lock_class!())
    };
}
            pub use new_spinlock_irq;

            pub type SpinLockIrq<T> = super::Lock<T, SpinLockIrqBackend>;

            pub struct SpinLockIrqBackend;

            unsafe impl super::Backend for SpinLockIrqBackend {
                type State = bindings::spinlock_t;
                type GuardState = core::ffi::c_ulong;

                #[inline]
                unsafe fn init(
                    ptr: *mut Self::State,
                    name: *const c_char,
                    key: *mut bindings::lock_class_key,
                ) {
                    unsafe { bindings::__spin_lock_init(ptr, name, key) }
                }

                #[inline]
                unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
                    unsafe { bindings::spin_lock_irq_disable(ptr) }
                }

                #[inline]
                unsafe fn unlock(ptr: *mut Self::State, guard_state: &Self::GuardState) {
                    unsafe { bindings::spin_unlock_irq_enable(ptr, *guard_state) }
                }
            }
        }

        pub unsafe trait Backend {
            type State;

            type GuardState;

            unsafe fn init(
                ptr: *mut Self::State,
                name: *const core::ffi::c_char,
                key: *mut bindings::lock_class_key,
            );

            #[must_use]
            unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState;

            unsafe fn unlock(ptr: *mut Self::State, guard_state: &Self::GuardState);

            unsafe fn relock(ptr: *mut Self::State, guard_state: &mut Self::GuardState) {
                *guard_state = unsafe { Self::lock(ptr) };
            }
        }

        #[repr(C)]
        #[pin_data]
        pub struct Lock<T: ?Sized, B: Backend> {
            #[pin]
            state: Opaque<B::State>,

            #[pin]
            _pin: PhantomPinned,

            pub(crate) data: UnsafeCell<T>,
        }

        impl<B: Backend> Lock<(), B> {
            pub unsafe fn from_raw<'a>(ptr: *mut B::State) -> &'a Self {
                unsafe { &*ptr.cast() }
            }
        }

        unsafe impl<T: ?Sized + Send, B: Backend> Send for Lock<T, B> {}

        unsafe impl<T: ?Sized + Send, B: Backend> Sync for Lock<T, B> {}

        impl<T, B: Backend> Lock<T, B> {
            #[allow(clippy::new_ret_no_self)]
            pub fn new(
                t: T,
                name: &'static CStr,
                key: &'static LockClassKey,
            ) -> impl PinInit<Self> {
                pin_init!(Self {
                    data: UnsafeCell::new(t),
                    _pin: PhantomPinned,
                    state <- Opaque::ffi_init(|slot| unsafe {
                        B::init(slot, name.as_char_ptr(), key.as_ptr())
                    }),
                })
            }
        }

        impl<T: ?Sized, B: Backend> Lock<T, B> {
            pub fn lock(&self) -> Guard<'_, T, B> {
                let state = unsafe { B::lock(self.state.get()) };
                unsafe { Guard::new(self, state) }
            }
        }

        #[must_use = "the lock unlocks immediately when the guard is unused"]
        pub struct Guard<'a, T: ?Sized, B: Backend> {
            pub(crate) lock: &'a Lock<T, B>,
            pub(crate) state: B::GuardState,
            _not_send: PhantomData<*mut ()>,
        }

        unsafe impl<T: Sync + ?Sized, B: Backend> Sync for Guard<'_, T, B> {}

        impl<T: ?Sized, B: Backend> Guard<'_, T, B> {
            pub(crate) fn do_unlocked(&mut self, cb: impl FnOnce()) {
                unsafe { B::unlock(self.lock.state.get(), &self.state) };

                let _relock = ScopeGuard::new(|| unsafe {
                    B::relock(self.lock.state.get(), &mut self.state)
                });

                cb();
            }
        }

        impl<T: ?Sized, B: Backend> core::ops::Deref for Guard<'_, T, B> {
            type Target = T;

            fn deref(&self) -> &Self::Target {
                unsafe { &*self.lock.data.get() }
            }
        }

        impl<T: ?Sized, B: Backend> core::ops::DerefMut for Guard<'_, T, B> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *self.lock.data.get() }
            }
        }

        impl<T: ?Sized, B: Backend> Drop for Guard<'_, T, B> {
            fn drop(&mut self) {
                unsafe { B::unlock(self.lock.state.get(), &self.state) };
            }
        }

        impl<'a, T: ?Sized, B: Backend> Guard<'a, T, B> {
            pub(crate) unsafe fn new(lock: &'a Lock<T, B>, state: B::GuardState) -> Self {
                Self {
                    lock,
                    state,
                    _not_send: PhantomData,
                }
            }
        }
    }
    pub mod lockdep {
        use crate::types::Opaque;
        #[repr(transparent)]
        pub struct LockClassKey(Opaque<bindings::lock_class_key>);
        unsafe impl Sync for LockClassKey {}
        impl LockClassKey {
            pub fn as_ptr(&self) -> *mut bindings::lock_class_key {
                self.0.get()
            }
        }
        #[doc(hidden)]
        #[macro_export]
        macro_rules! static_lock_class {
            () => {{
                static CLASS: $crate::sync::LockClassKey =
                    unsafe { ::core::mem::MaybeUninit::uninit().assume_init() };
                &CLASS
            }};
        }
        pub use static_lock_class;
        #[doc(hidden)]
        #[macro_export]
        macro_rules! optional_name {
            () => {
                $crate::c_str!(::core::concat!(::core::file!(), ":", ::core::line!()))
            };
            ($name:literal) => {
                $crate::c_str!($name)
            };
        }
        #[cfg(CONFIG_DEBUG_LOCK_ALLOC)]
        pub const fn raw_lockdep_map(
            name: &'static crate::str::CStr,
            wait_type_inner: u8,
            wait_type_outer: u8,
        ) -> bindings::lockdep_map {
            let mut map: bindings::lockdep_map = unsafe { core::mem::zeroed() };
            map.name = name.as_char_ptr();
            map.wait_type_inner = wait_type_inner;
            map.wait_type_outer = wait_type_outer;
            map
        }
    }
    pub mod set_once {
        use core::{
            cell::UnsafeCell,
            mem::MaybeUninit,
            sync::atomic::{
                AtomicU32,
                Ordering::{Acquire, Relaxed, Release},
            },
        };
        pub struct SetOnce<T> {
            init: AtomicU32,
            value: UnsafeCell<MaybeUninit<T>>,
        }
        impl<T> Default for SetOnce<T> {
            fn default() -> Self {
                Self::new()
            }
        }
        impl<T> SetOnce<T> {
            pub const fn new() -> Self {
                Self {
                    value: UnsafeCell::new(MaybeUninit::uninit()),
                    init: AtomicU32::new(0),
                }
            }
            pub fn as_ref(&self) -> Option<&T> {
                if self.init.load(Acquire) == 2 {
                    Some(unsafe { &*self.value.get().cast() })
                } else {
                    None
                }
            }
            pub fn populate(&self, value: T) -> bool {
                if let Ok(0) = self.init.compare_exchange(0, 1, Relaxed, Relaxed) {
                    unsafe { core::ptr::write(self.value.get().cast(), value) };
                    self.init.store(2, Release);
                    true
                } else {
                    false
                }
            }
        }
        impl<T> Drop for SetOnce<T> {
            fn drop(&mut self) {
                if *self.init.get_mut() == 2 {
                    let value = self.value.get_mut();
                    unsafe { value.assume_init_drop() };
                }
            }
        }
        unsafe impl<T: Send> Send for SetOnce<T> {}
        unsafe impl<T: Send + Sync> Sync for SetOnce<T> {}
    }
    pub use crate::new_mutex;
    pub use arc::{Arc, ArcBorrow, UniqueArc};
    pub use completion::Completion;
    pub use lock::mutex::Mutex;
    pub use lock::spinlock::{new_spinlock, new_spinlock_irq, SpinLock, SpinLockIrq};
    pub use lock::{Backend, Guard, Lock};
    pub use lockdep::LockClassKey;
    pub(crate) use set_once::SetOnce;
    #[repr(transparent)]
    pub struct RefCount(Opaque<bindings::refcount_t>);
    impl RefCount {
        #[inline]
        pub fn new(val: i32) -> Self {
            Self(Opaque::new(unsafe { bindings::REFCOUNT_INIT(val) }))
        }
        #[inline]
        pub fn inc(&self) {
            unsafe { bindings::refcount_inc(self.0.get()) }
        }
        #[inline]
        pub fn dec_and_test(&self) -> bool {
            unsafe { bindings::refcount_dec_and_test(self.0.get()) }
        }
    }
}

pub trait Module: Sized + Sync + Send {
    fn init(module: &'static ThisModule) -> error::Result<Self>;
}

pub struct ThisModule(*mut bindings::module);
unsafe impl Sync for ThisModule {}
impl ThisModule {
    pub const unsafe fn from_ptr(ptr: *mut bindings::module) -> ThisModule {
        ThisModule(ptr)
    }
    pub const fn as_ptr(&self) -> *mut bindings::module {
        self.0
    }
}

#[cfg(not(any(testlib, test)))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    pr_emerg!("{}\n", info);
    loop {}
}

#[macro_export]
macro_rules! container_of {
    ($ptr:expr, $type:ty, $($f:tt)*) => {{
        let ptr = $ptr as *const _ as *const u8;
        let offset: usize = ::core::mem::offset_of!($type, $($f)*);
        ptr.sub(offset) as *const $type
    }}
}

pub const __LOG_PREFIX: &[u8] = b"rust_kernel\0";
