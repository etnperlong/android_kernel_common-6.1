// SPDX-License-Identifier: GPL-2.0
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
