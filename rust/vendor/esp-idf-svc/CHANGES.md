# Vendored `esp-idf-svc` 0.48.1

This is an unmodified copy of `esp-idf-svc` 0.48.1 except for the change below,
wired in via `[patch.crates-io]` in `rust/Cargo.toml`.

## Why

Rust's `core::ffi::c_char` for the `xtensa-esp32-espidf` target is now **`u8`**
(unsigned) in the current Espressif toolchain, and `bindgen` emits `c_char` for
C `char` unconditionally. `esp-idf-svc` 0.48.1 hardcodes `*const i8` / `*mut i8`
for the affected `char *` arguments, so it no longer type-checks (`E0308`).

Using `c_char` instead of `i8` is exactly the fix upstream applied in
`esp-idf-svc` 0.50.x; staying on the 0.48.1 line preserves ESP-IDF v4.4 support
(the emulator's Wi-Fi radio only runs under 4.4).

## Diff vs. crates.io 0.48.1

- `src/private/cstr.rs`: `cstr_arr_from_str_slice` returns `[*const c_char; N]`.
- `src/tls.rs`: `RawConfigBufs::alpn_protos` is `[*const c_char; 10]`; the four
  `esp_tls_conn_new_{async,sync}` / `read` / `write` casts use `c_char`.

Also removed `examples/` to keep the tree small (not built as a dependency).

To refresh after a version bump: re-copy the crate from the registry and reapply
the cast changes above.
