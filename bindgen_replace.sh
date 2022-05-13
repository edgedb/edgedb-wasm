#!/usr/bin/bash
set -ex -o pipefail
grep -R '^wit_bindgen_rust' sdk | while read line; do
    filename="${line%%:*}"
    _after_quote="${line#*\"}"
    witfile="${_after_quote%\"*}"
    _after_colon="${line##*:}"
    mode="${_after_colon%\!*}"
    wit-bindgen rust-wasm --$mode "sdk/$witfile"
    sed -i '
        s/wit_bindgen_rust::bitflags/bitflags/g
        s/wit_bindgen_rust::rt::as_i/crate::bindgen::as_i/g
        s/impl wit_bindgen_rust::rt::AsI/impl crate::bindgen::AsI/g
    ' bindings.rs
    sed -i '
        /wit_bindgen_rust/r bindings.rs
        /wit_bindgen_rust/i #[allow(dead_code)]
        /wit_bindgen_rust/i #[allow(unused_parens)]
        /wit_bindgen_rust/s/^/\/\/ /g
    ' "$filename"
done
rm bindings.rs

sed -i '
    /^#\[cfg(feature="bindgen")\]/D
' sdk/src/lib.rs

sed -i '
    /^wit-bindgen-rust/i bitflags = "1.3.2"
    /^wit-bindgen-rust/D
' sdk/Cargo.toml

