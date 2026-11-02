// SPDX-License-Identifier: GPL-2.0
use core::fmt::{self, Debug, Formatter};
use kernel::prelude::*;
pub struct PushError<T>(pub T);
impl<T> Debug for PushError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Not enough capacity")
    }
}
impl<T> From<PushError<T>> for Error {
    fn from(_: PushError<T>) -> Error {
        EINVAL
    }
}
pub struct RemoveError;
impl Debug for RemoveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Index out of bounds")
    }
}
impl From<RemoveError> for Error {
    fn from(_: RemoveError) -> Error {
        EINVAL
    }
}
pub enum InsertError<T> {
    IndexOutOfBounds(T),
    OutOfCapacity(T),
}
impl<T> Debug for InsertError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            InsertError::IndexOutOfBounds(_) => write!(f, "Index out of bounds"),
            InsertError::OutOfCapacity(_) => write!(f, "Not enough capacity"),
        }
    }
}
impl<T> From<InsertError<T>> for Error {
    fn from(_: InsertError<T>) -> Error {
        EINVAL
    }
}
