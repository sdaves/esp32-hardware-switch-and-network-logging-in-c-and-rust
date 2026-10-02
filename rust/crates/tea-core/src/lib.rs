//! Pure TEA core: `update(Model, Msg) -> UpdateResult { next, command }`.
//! `no_std`, no heap, no I/O, no `unsafe`. Unit-tested on the host.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod use_cases;
