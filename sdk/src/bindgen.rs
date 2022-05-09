//! This module replaces `wit_bindgen_rust::rt` when we use pregenerated
//! code instead of macros.
//!
//! (this is needed to publish generated code instead of relying on unpublished
//! git version of wit-bindgen)
#![allow(dead_code)]

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
