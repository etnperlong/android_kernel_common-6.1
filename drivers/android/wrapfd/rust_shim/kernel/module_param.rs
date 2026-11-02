// SPDX-License-Identifier: GPL-2.0
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
unsafe extern "C" fn set_param<T>(val: *const c_char, param: *const bindings::kernel_param) -> c_int
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
impl_int_module_param!(i8);
impl_int_module_param!(u8);
impl_int_module_param!(i16);
impl_int_module_param!(u16);
impl_int_module_param!(i32);
impl_int_module_param!(u32);
impl_int_module_param!(i64);
impl_int_module_param!(u64);
impl_int_module_param!(isize);
impl_int_module_param!(usize);
unsafe extern "C" fn get_param<T>(buf: *mut c_char, param: *const bindings::kernel_param) -> c_int
where
    T: core::fmt::Display,
{
    if buf.is_null() {
        crate::pr_warn!("Null pointer passed to `module_param::get_param`");
        return EINVAL.to_errno();
    }
    let mut writer = unsafe { RawFormatter::from_buffer(buf, PAGE_SIZE - 1) };
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
macro_rules! make_param_ops {
    ($ops:ident, $ty:ty) => {
        #[doc(hidden)]
        pub static $ops: $crate::bindings::kernel_param_ops = $crate::bindings::kernel_param_ops {
            flags: 0,
            set: Some(set_param::<$ty>),
            get: Some(get_param::<$ty>),
            free: None,
        };
    };
}
make_param_ops!(PARAM_OPS_I8, i8);
make_param_ops!(PARAM_OPS_U8, u8);
make_param_ops!(PARAM_OPS_I16, i16);
make_param_ops!(PARAM_OPS_U16, u16);
make_param_ops!(PARAM_OPS_I32, i32);
make_param_ops!(PARAM_OPS_U32, u32);
make_param_ops!(PARAM_OPS_I64, i64);
make_param_ops!(PARAM_OPS_U64, u64);
make_param_ops!(PARAM_OPS_ISIZE, isize);
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
pub static PARAM_OPS_USIZE: crate::bindings::kernel_param_ops = crate::bindings::kernel_param_ops {
    flags: 0,
    set: Some(crate::bindings::kcfi_param_set_usize),
    get: Some(crate::bindings::kcfi_param_get_usize),
    free: None,
};
