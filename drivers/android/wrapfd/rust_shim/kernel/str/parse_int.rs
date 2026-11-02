// SPDX-License-Identifier: GPL-2.0
use crate::prelude::*;
use crate::str::BStr;
use core::ops::Deref;
mod private {
    use crate::prelude::*;
    use crate::str::BStr;
    pub trait FromStrRadix: Sized {
        fn from_str_radix(src: &BStr, radix: u32) -> Result<Self>;
        fn from_u64_negated(value: u64) -> Result<Self>;
    }
}
fn strip_radix(src: &BStr) -> (u32, &BStr) {
    match src.deref() {
        [b'0', b'x' | b'X', rest @ ..] => (16, rest.as_ref()),
        [b'0', b'o' | b'O', rest @ ..] => (8, rest.as_ref()),
        [b'0', b'b' | b'B', rest @ ..] => (2, rest.as_ref()),
        [b'0', ..] => (8, src),
        _ => (10, src),
    }
}
pub trait ParseInt: private::FromStrRadix + TryFrom<u64> {
    fn from_str(src: &BStr) -> Result<Self> {
        match src.deref() {
            [b'-', rest @ ..] => {
                let (radix, digits) = strip_radix(rest.as_ref());
                let val =
                    u64::from_str_radix(core::str::from_utf8(digits).map_err(|_| EINVAL)?, radix)
                        .map_err(|_| EINVAL)?;
                Self::from_u64_negated(val)
            }
            _ => {
                let (radix, digits) = strip_radix(src);
                Self::from_str_radix(digits, radix).map_err(|_| EINVAL)
            }
        }
    }
}
macro_rules! impl_parse_int {
    ($($ty:ty),*) => {
        $(
            impl private::FromStrRadix for $ty {
                fn from_str_radix(src: &BStr, radix: u32) -> Result<Self> {
                    <$ty>::from_str_radix(core::str::from_utf8(src).map_err(|_| EINVAL)?, radix)
                        .map_err(|_| EINVAL)
                }
                fn from_u64_negated(value: u64) -> Result<Self> {
                    const ABS_MIN: u64 = {
                        #[allow(unused_comparisons)]
                        if <$ty>::MIN < 0 {
                            1u64 << (<$ty>::BITS - 1)
                        } else {
                            0
                        }
                    };
                    if value > ABS_MIN {
                        return Err(EINVAL);
                    }
                    if value == ABS_MIN {
                        return Ok(<$ty>::MIN);
                    }
                    let value: Self = unsafe { value.try_into().unwrap_unchecked() };
                    Ok((!value).wrapping_add(1))
                }
            }
            impl ParseInt for $ty {}
        )*
    };
}
impl_parse_int![i8, u8, i16, u16, i32, u32, i64, u64, isize, usize];
