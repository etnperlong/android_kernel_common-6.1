// SPDX-License-Identifier: Apache-2.0 OR MIT
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
