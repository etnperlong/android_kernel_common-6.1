// SPDX-License-Identifier: GPL-2.0
use crate::{
    bindings,
    ffi::{c_int, c_long, c_uint},
    mm::MmWithUser,
    types::{ARef, NotThreadSafe, Opaque},
};
use core::{
    cmp::{Eq, PartialEq},
    ops::Deref,
    ptr,
};
pub const MAX_SCHEDULE_TIMEOUT: c_long = c_long::MAX;
pub const TASK_INTERRUPTIBLE: c_int = bindings::TASK_INTERRUPTIBLE as c_int;
pub const TASK_UNINTERRUPTIBLE: c_int = bindings::TASK_UNINTERRUPTIBLE as c_int;
pub const TASK_FREEZABLE: c_int = bindings::TASK_FREEZABLE as c_int;
pub const TASK_NORMAL: c_uint = bindings::TASK_NORMAL as c_uint;
#[macro_export]
macro_rules! current {
    () => {
        unsafe { &*$crate::task::Task::current() }
    };
}
#[repr(transparent)]
pub struct Task(pub(crate) Opaque<bindings::task_struct>);
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
impl CurrentTask {
    #[inline]
    pub fn mm(&self) -> Option<&MmWithUser> {
        let mm = unsafe { (*self.as_ptr()).mm };
        if mm.is_null() {
            return None;
        }
        Some(unsafe { MmWithUser::from_raw(mm) })
    }
}
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
impl Kuid {
    #[inline]
    pub fn current_euid() -> Kuid {
        Self::from_raw(unsafe { bindings::current_euid() })
    }
    #[inline]
    pub fn from_raw(kuid: bindings::kuid_t) -> Self {
        Self { kuid }
    }
    #[inline]
    pub fn into_raw(self) -> bindings::kuid_t {
        self.kuid
    }
    #[inline]
    pub fn into_uid_in_current_ns(self) -> bindings::uid_t {
        unsafe { bindings::from_kuid(bindings::current_user_ns(), self.kuid) }
    }
}
impl PartialEq for Kuid {
    #[inline]
    fn eq(&self, other: &Kuid) -> bool {
        unsafe { bindings::uid_eq(self.kuid, other.kuid) }
    }
}
impl Eq for Kuid {}
