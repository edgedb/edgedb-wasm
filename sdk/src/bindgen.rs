//! This module replaces `wit_bindgen_rust::rt` when we use pregenerated
//! code instead of macros.
//!
//! (this is needed to publish generated code instead of relying on unpublished
//! git version of wit-bindgen)
#![allow(dead_code)]

use std::alloc::{self, Layout};

pub trait AsI32 {
    fn as_i32(self) -> i32;
}

pub trait AsI64 {
    fn as_i64(self) -> i64;
}

pub fn as_i32<T: AsI32>(t: T) -> i32 {
    t.as_i32()
}

pub fn as_i64<T: AsI64>(t: T) -> i64 {
    t.as_i64()
}

impl AsI32 for i32 {
    fn as_i32(self) -> i32 {
        self
    }
}

impl AsI32 for u32 {
    fn as_i32(self) -> i32 {
        self as i32
    }
}

impl AsI32 for u16 {
    fn as_i32(self) -> i32 {
        self as i32
    }
}

impl AsI64 for i64 {
    fn as_i64(self) -> i64 {
        self
    }
}

impl AsI64 for u64 {
    fn as_i64(self) -> i64 {
        self as i64
    }
}

#[no_mangle]
pub unsafe extern "C" fn canonical_abi_free(ptr: *mut u8, len: usize, align: usize) {
    if len == 0 {
        return;
    }
    let layout = Layout::from_size_align_unchecked(len, align);
    alloc::dealloc(ptr, layout);
}

#[no_mangle]
unsafe extern "C" fn canonical_abi_realloc(
    old_ptr: *mut u8,
    old_len: usize,
    align: usize,
    new_len: usize,
) -> *mut u8 {
    let layout;
    let ptr = if old_len == 0 {
        if new_len == 0 {
            return align as *mut u8;
        }
        layout = Layout::from_size_align_unchecked(new_len, align);
        alloc::alloc(layout)
    } else {
        layout = Layout::from_size_align_unchecked(old_len, align);
        alloc::realloc(old_ptr, layout, new_len)
    };
    if ptr.is_null() {
        alloc::handle_alloc_error(layout);
    }
    return ptr;
}
