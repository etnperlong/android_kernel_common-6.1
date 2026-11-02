// SPDX-License-Identifier: Apache-2.0 OR MIT
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
        @decl_generics($($decl_generics:tt)*),
        @body({ $($fields:tt)* }),
    ) => {
        $crate::__pin_data!(find_pinned_fields:
            @struct_attrs($(#[$($struct_attr)*])*),
            @vis($vis),
            @name($name),
            @impl_generics($($impl_generics)*),
            @ty_generics($($ty_generics)*),
            @decl_generics($($decl_generics)*),
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
        @decl_generics($($decl_generics:tt)*),
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
            @decl_generics($($decl_generics)*),
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
        @decl_generics($($decl_generics:tt)*),
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
            @decl_generics($($decl_generics)*),
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
        @decl_generics($($decl_generics:tt)*),
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
            @decl_generics($($decl_generics)*),
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
        @decl_generics($($decl_generics:tt)*),
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
            @decl_generics($($decl_generics)*),
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
        @decl_generics($($decl_generics:tt)*),
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
            @decl_generics($($decl_generics)*),
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
        @decl_generics($($decl_generics:tt)*),
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
            @decl_generics($($decl_generics)*),
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
        @decl_generics($($decl_generics:tt)*),
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
        $vis struct $name <$($decl_generics)*>
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
        #[allow(dead_code)]
        trait MustNotImplDrop {}
        #[expect(drop_bounds)]
        impl<T: ::core::ops::Drop> MustNotImplDrop for T {}
        impl<$($impl_generics)*> MustNotImplDrop for $name<$($ty_generics)*>
        where $($whr)* {}
        #[allow(dead_code)]
        #[expect(non_camel_case_types)]
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
        $crate::macros::paste! {
            #[allow(dead_code, non_snake_case)]
            #[expect(clippy::missing_safety_doc)]
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
                    $(#[$($p_attr)*])*
                    $pvis unsafe fn [<__project_ $p_field>]<'__slot>(
                        self,
                        slot: &'__slot mut $p_type,
                    ) -> ::core::pin::Pin<&'__slot mut $p_type> {
                        unsafe { ::core::pin::Pin::new_unchecked(slot) }
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
                    $(#[$($attr)*])*
                    $fvis unsafe fn [<__project_ $field>]<'__slot>(
                        self,
                        slot: &'__slot mut $type,
                    ) -> &'__slot mut $type {
                        slot
                    }
                )*
            }
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
        let _ = unsafe { &mut (*$slot).$field };
        ::kernel::macros::paste! {
            let mut [< __ $field _guard >] = unsafe {
                $crate::init::__internal::DropGuard::new(::core::ptr::addr_of_mut!((*$slot).$field))
            };
            #[allow(unused_variables, unused_assignments)]
            let $field = unsafe {
                $data.[< __project_ $field >]([< __ $field _guard >].let_binding())
            };
            $crate::__init_internal!(init_slot($use_data):
                @data($data),
                @slot($slot),
                @guards([< __ $field _guard >], $($guards,)*),
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
        let _ = unsafe { &mut (*$slot).$field };
        ::kernel::macros::paste! {
            let mut [< __ $field _guard >] = unsafe {
                $crate::init::__internal::DropGuard::new(::core::ptr::addr_of_mut!((*$slot).$field))
            };
            #[allow(unused_variables, unused_assignments)]
            let $field = [< __ $field _guard >].let_binding();
            $crate::__init_internal!(init_slot():
                @data($data),
                @slot($slot),
                @guards([< __ $field _guard >], $($guards,)*),
                @munch_fields($($rest)*),
            );
        }
    };
    (init_slot(): // No `use_data`, so all fields are not structurally pinned
        @data($data:ident),
        @slot($slot:ident),
        @guards($($guards:ident,)*),
        @munch_fields($field:ident $(: $val:expr)?, $($rest:tt)*),
    ) => {
        {
            $(let $field = $val;)?
            unsafe { ::core::ptr::write(::core::ptr::addr_of_mut!((*$slot).$field), $field) };
        }
        let _ = unsafe { &mut (*$slot).$field };
        ::kernel::macros::paste! {
            let mut [< __ $field _guard >] = unsafe {
                $crate::init::__internal::DropGuard::new(::core::ptr::addr_of_mut!((*$slot).$field))
            };
            #[allow(unused_variables, unused_assignments)]
            let $field = [< __ $field _guard >].let_binding();
            $crate::__init_internal!(init_slot():
                @data($data),
                @slot($slot),
                @guards([< __ $field _guard >], $($guards,)*),
                @munch_fields($($rest)*),
            );
        }
    };
    (init_slot($use_data:ident):
        @data($data:ident),
        @slot($slot:ident),
        @guards($($guards:ident,)*),
        @munch_fields($field:ident $(: $val:expr)?, $($rest:tt)*),
    ) => {
        {
            $(let $field = $val;)?
            unsafe { ::core::ptr::write(::core::ptr::addr_of_mut!((*$slot).$field), $field) };
        }
        let _ = unsafe { &mut (*$slot).$field };
        $crate::macros::paste! {
            let mut [< __ $field _guard >] = unsafe {
                $crate::init::__internal::DropGuard::new(::core::ptr::addr_of_mut!((*$slot).$field))
            };
            #[allow(unused_variables, unused_assignments)]
            let $field = unsafe {
                $data.[< __project_ $field >]([< __ $field _guard >].let_binding())
            };
            $crate::__init_internal!(init_slot($use_data):
                @data($data),
                @slot($slot),
                @guards([< __ $field _guard >], $($guards,)*),
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
