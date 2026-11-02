#!/bin/bash
set -e

WRAPFD_SRC="$1"
WRAPFD_OUT="$2"
shift 2
BINDGEN_CFLAGS=("$@")

WRAPFD_SHIM="$WRAPFD_SRC/rust_shim"
export WRAPFD_RUST_OUT="$WRAPFD_OUT"
export RUSTC_BOOTSTRAP=1

COMMON_FLAGS=(
  --edition=2021 -Zbinary_dep_depinfo=y
  -Astable_features -Aunused_features -Amissing_docs -Adead_code -Aunused_imports
  -Dnon_ascii_idents -Dunsafe_op_in_unsafe_fn
  -Wrust_2018_idioms -Wunreachable_pub
)

TARGET_FLAGS=(
  "${COMMON_FLAGS[@]}"
  -Cpanic=abort -Cembed-bitcode=n -Clto=n
  -Cforce-unwind-tables=n -Ccodegen-units=1
  -Csymbol-mangling-version=v0 -Crelocation-model=static
  -Zfunction-sections=y -Dclippy::float_arithmetic
  --target=aarch64-unknown-none -Ctarget-feature=-neon
  -Zfixed-x18 -Copt-level=2 -Cdebug-assertions=n
  -Coverflow-checks=y -Dwarnings -Cforce-frame-pointers=y
  -Cdebuginfo=0
  --cfg CONFIG_RUST --cfg CONFIG_ARM64 --cfg CONFIG_64BIT --cfg CONFIG_MMU
  --cfg CONFIG_SMP --cfg CONFIG_COMPAT --cfg CONFIG_PROC_FS --cfg CONFIG_HAVE_ARCH_PREL32_RELOCATIONS
)

mapfile -t BPARAMS < <(grep -v '^#' "$WRAPFD_SHIM/bindgen_parameters" | tr '\n' ' ')
BEXTRA=(
  --rust-target 1.68 --use-core --with-derive-default
  --ctypes-prefix ffi --no-layout-tests --no-debug '.*'
  --enable-function-attribute-detection
)

if [ ! -f "$WRAPFD_OUT/bindings_generated.rs" ]; then
  bindgen "$WRAPFD_SHIM/bindings/bindings_helper.h" \
    ${BPARAMS[@]} "${BEXTRA[@]}" \
    -o "$WRAPFD_OUT/bindings_generated.rs" -- "${BINDGEN_CFLAGS[@]}"
  sed -Ei 's/pub const RUST_CONST_HELPER_([a-zA-Z0-9_]*)/pub const \1/g' "$WRAPFD_OUT/bindings_generated.rs"
fi

bindgen "$WRAPFD_SHIM/helpers/helpers.c" \
  --blocklist-type '.*' --allowlist-var '' --allowlist-function 'rust_helper_.*' \
  ${BPARAMS[@]} "${BEXTRA[@]}" \
  -o "$WRAPFD_OUT/bindings_helpers_generated.rs" -- "${BINDGEN_CFLAGS[@]}" -I"$WRAPFD_SHIM/helpers"
sed -Ei 's/pub fn rust_helper_([a-zA-Z0-9_]*)/#[link_name="rust_helper_\1"]\n\tpub fn \1/g' "$WRAPFD_OUT/bindings_helpers_generated.rs"

if [ ! -f "$WRAPFD_OUT/uapi_generated.rs" ]; then
  bindgen "$WRAPFD_SHIM/uapi/uapi_helper.h" \
    ${BPARAMS[@]} "${BEXTRA[@]}" \
    -o "$WRAPFD_OUT/uapi_generated.rs" -- "${BINDGEN_CFLAGS[@]}"
fi

if [ ! -f "$WRAPFD_OUT/libmacros.so" ]; then
  rustc "${COMMON_FLAGS[@]}" -Clinker-flavor=gcc -Clinker=clang \
    --emit=link="$WRAPFD_OUT/libmacros.so" --extern proc_macro \
    --crate-type proc-macro --crate-name macros "$WRAPFD_SHIM/macros/lib.rs"
fi

if [ ! -f "$WRAPFD_OUT/core.o" ]; then
  CORESRC="$(rustc --print sysroot)/lib/rustlib/src/rust/library/core/src/lib.rs"
  rustc "${TARGET_FLAGS[@]}" -Aunreachable_pub \
    --emit=obj="$WRAPFD_OUT/core.o" --emit=metadata="$WRAPFD_OUT/libcore.rmeta" \
    --crate-type rlib --crate-name core "$CORESRC" --sysroot=/dev/null -Zunstable-options
  llvm-objcopy \
    --redefine-sym __adddf3=__rust__adddf3 --redefine-sym __addsf3=__rust__addsf3 \
    --redefine-sym __eqdf2=__rust__eqdf2 --redefine-sym __eqsf2=__rust__eqsf2 \
    --redefine-sym __extendsfdf2=__rust__extendsfdf2 --redefine-sym __gesf2=__rust__gesf2 \
    --redefine-sym __ledf2=__rust__ledf2 --redefine-sym __lesf2=__rust__lesf2 \
    --redefine-sym __ltdf2=__rust__ltdf2 --redefine-sym __ltsf2=__rust__ltsf2 \
    --redefine-sym __muldf3=__rust__muldf3 --redefine-sym __mulsf3=__rust__mulsf3 \
    --redefine-sym __nesf2=__rust__nesf2 --redefine-sym __truncdfsf2=__rust__truncdfsf2 \
    --redefine-sym __unorddf2=__rust__unorddf2 --redefine-sym __unordsf2=__rust__unordsf2 \
    --redefine-sym __muloti4=__rust__muloti4 --redefine-sym __multi3=__rust__multi3 \
    --redefine-sym __udivmodti4=__rust__udivmodti4 --redefine-sym __udivti3=__rust__udivti3 \
    --redefine-sym __umodti3=__rust__umodti3 --redefine-sym __ashrti3=__rust__ashrti3 \
    --redefine-sym __ashlti3=__rust__ashlti3 --redefine-sym __lshrti3=__rust__lshrti3 \
    "$WRAPFD_OUT/core.o"
fi

if [ ! -f "$WRAPFD_OUT/compiler_builtins.o" ]; then
  rustc "${TARGET_FLAGS[@]}" \
    --emit=obj="$WRAPFD_OUT/compiler_builtins.o" --emit=metadata="$WRAPFD_OUT/libcompiler_builtins.rmeta" \
    --crate-type rlib -L"$WRAPFD_OUT" --crate-name compiler_builtins "$WRAPFD_SHIM/compiler_builtins.rs" --sysroot=/dev/null -Zunstable-options
  llvm-objcopy -w -W '__*' "$WRAPFD_OUT/compiler_builtins.o"
fi

if [ ! -f "$WRAPFD_OUT/ffi.o" ]; then
  rustc "${TARGET_FLAGS[@]}" \
    --emit=obj="$WRAPFD_OUT/ffi.o" --emit=metadata="$WRAPFD_OUT/libffi.rmeta" \
    --crate-type rlib -L"$WRAPFD_OUT" --crate-name ffi "$WRAPFD_SHIM/ffi.rs" --sysroot=/dev/null -Zunstable-options
fi

if [ ! -f "$WRAPFD_OUT/build_error.o" ]; then
  rustc "${TARGET_FLAGS[@]}" \
    --emit=obj="$WRAPFD_OUT/build_error.o" --emit=metadata="$WRAPFD_OUT/libbuild_error.rmeta" \
    --crate-type rlib -L"$WRAPFD_OUT" --crate-name build_error "$WRAPFD_SHIM/build_error.rs" --sysroot=/dev/null -Zunstable-options
fi

rustc "${TARGET_FLAGS[@]}" --extern ffi \
  --emit=obj="$WRAPFD_OUT/bindings.o" --emit=metadata="$WRAPFD_OUT/libbindings.rmeta" \
  --crate-type rlib -L"$WRAPFD_OUT" --crate-name bindings "$WRAPFD_SHIM/bindings/lib.rs" --sysroot=/dev/null -Zunstable-options

rustc "${TARGET_FLAGS[@]}" --extern ffi \
  --emit=obj="$WRAPFD_OUT/uapi.o" --emit=metadata="$WRAPFD_OUT/libuapi.rmeta" \
  --crate-type rlib -L"$WRAPFD_OUT" --crate-name uapi "$WRAPFD_SHIM/uapi/lib.rs" --sysroot=/dev/null -Zunstable-options

rustc "${TARGET_FLAGS[@]}" --extern ffi --extern build_error --extern macros --extern bindings --extern uapi \
  --emit=obj="$WRAPFD_OUT/kernel.o" --emit=metadata="$WRAPFD_OUT/libkernel.rmeta" \
  --crate-type rlib -L"$WRAPFD_OUT" --crate-name kernel "$WRAPFD_SHIM/kernel/lib.rs" --sysroot=/dev/null -Zunstable-options

RUST_MODFILE=drivers/android/wrapfd/rust_wrapfd rustc "${TARGET_FLAGS[@]}" \
  -Zcrate-attr=no_std -Zcrate-attr='feature(arbitrary_self_types,asm_const,asm_goto,used_with_arg)' \
  -Zunstable-options --extern kernel --crate-type rlib -L"$WRAPFD_OUT" --sysroot=/dev/null \
  --emit=obj="$WRAPFD_OUT/rust_wrapfd.o" "$WRAPFD_SRC/rust_wrapfd.rs"
