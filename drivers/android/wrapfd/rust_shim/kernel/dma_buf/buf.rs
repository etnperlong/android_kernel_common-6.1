// SPDX-License-Identifier: GPL-2.0
use crate::{
    bindings,
    device::Device,
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
    pub fn into_fd(this: ARef<Self>, flags: i32) -> Result<i32> {
        let ptr = ARef::into_raw(this);
        let fd = unsafe { bindings::dma_buf_fd(ptr.as_ptr().cast(), flags) };
        if fd < 0 {
            drop(unsafe { ARef::from_raw(ptr) });
            Err(Error::from_errno(fd))
        } else {
            Ok(fd)
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> *mut bindings::dma_buf {
        self.0.get()
    }
    pub fn attach(&self, dev: &Device) -> Result<DmaBufAttachment> {
        let ptr = unsafe { bindings::dma_buf_attach(self.as_ptr(), dev.as_raw()) };
        let ptr = from_err_ptr(ptr)?;
        Ok(DmaBufAttachment {
            ptr: NonNull::new(ptr).ok_or(EINVAL)?,
            dmabuf: ARef::from(self),
        })
    }
    pub fn vmap(&self) -> Result<DmaBufVmap> {
        let mut map = bindings::iosys_map::default();
        let ret = unsafe { bindings::dma_buf_vmap_unlocked(self.as_ptr(), &raw mut map) };
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
pub struct DmaBufAttachment {
    ptr: NonNull<bindings::dma_buf_attachment>,
    dmabuf: ARef<DmaBuf>,
}
unsafe impl Send for DmaBufAttachment {}
unsafe impl Sync for DmaBufAttachment {}
impl DmaBufAttachment {
    pub fn map(self, dir: DataDirection) -> Result<DmaBufMapping> {
        let sgt =
            unsafe { bindings::dma_buf_map_attachment_unlocked(self.ptr.as_ptr(), dir.into()) };
        let sgt = from_err_ptr(sgt)?;
        let non_null = NonNull::new(sgt).ok_or(EINVAL)?;
        Ok(DmaBufMapping {
            attachment: self,
            sgt: non_null,
            dir,
        })
    }
}
impl Drop for DmaBufAttachment {
    fn drop(&mut self) {
        unsafe { bindings::dma_buf_detach(self.dmabuf.as_ptr(), self.ptr.as_ptr()) };
    }
}
pub struct DmaBufMapping {
    attachment: DmaBufAttachment,
    sgt: NonNull<bindings::sg_table>,
    dir: DataDirection,
}
unsafe impl Send for DmaBufMapping {}
unsafe impl Sync for DmaBufMapping {}
impl Drop for DmaBufMapping {
    fn drop(&mut self) {
        unsafe {
            bindings::dma_buf_unmap_attachment_unlocked(
                self.attachment.ptr.as_ptr(),
                self.sgt.as_ptr(),
                self.dir.into(),
            )
        };
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
    pub fn dmabuf(&self) -> &DmaBuf {
        &self.dmabuf
    }
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
        to_result(unsafe { bindings::dma_buf_begin_cpu_access(self.dmabuf.as_ptr(), dir.into()) })?;
        let result = f(CpuAccess {
            map: &raw mut map,
            size: self.dmabuf.size(),
            _lifetime: PhantomData,
        });
        to_result(unsafe { bindings::dma_buf_end_cpu_access(self.dmabuf.as_ptr(), dir.into()) })?;
        Ok(result)
    }
    pub fn begin_cpu_access_to_fallible<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: FnOnce(CpuAccess<'_, false, true>) -> Result<T, E>,
        E: From<Error>,
    {
        self.begin_cpu_access_fallible(f)
    }
    pub fn begin_cpu_access_to<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(CpuAccess<'_, false, true>) -> T,
    {
        self.begin_cpu_access(f)
    }
    pub fn begin_cpu_access_from_fallible<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: FnOnce(CpuAccess<'_, true, false>) -> Result<T, E>,
        E: From<Error>,
    {
        self.begin_cpu_access_fallible(f)
    }
    pub fn begin_cpu_access_from<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(CpuAccess<'_, true, false>) -> T,
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
    pub fn begin_cpu_access_bidirectional<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(CpuAccess<'_, true, true>) -> T,
    {
        self.begin_cpu_access(f)
    }
}
impl Drop for DmaBufVmap {
    #[inline]
    fn drop(&mut self) {
        unsafe { bindings::dma_buf_vunmap_unlocked(self.dmabuf.as_ptr(), &mut self.map) };
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
        unsafe { bindings::iosys_map_memcpy_to(self.map, offset, src.as_ptr().cast(), src.len()) };
    }
    #[inline]
    #[track_caller]
    pub fn memset(&mut self, offset: usize, value: u8, len: usize) {
        let (end, end_overflow) = offset.overflowing_add(len);
        if end_overflow || end > self.size {
            panic!("memset of {len} bytes at {offset} in dma-buf out of bounds",);
        }
        unsafe { bindings::iosys_map_memset(self.map, offset, value as crate::ffi::c_int, len) };
    }
}
impl<'a, const WRITE: bool> CpuAccess<'a, true, WRITE> {
    #[inline]
    #[track_caller]
    pub fn memcpy_from(&mut self, dst: &mut [u8], offset: usize) {
        let (end, end_overflow) = offset.overflowing_add(dst.len());
        if end_overflow || end > self.size {
            panic!(
                "memcpy_from of {} bytes at {offset} in dma-buf of {} bytes out of bounds",
                dst.len(),
                self.size
            );
        }
        unsafe {
            bindings::iosys_map_memcpy_from(dst.as_mut_ptr().cast(), self.map, offset, dst.len())
        };
    }
}
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
