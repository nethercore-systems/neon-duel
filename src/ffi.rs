//! FFI bindings for ZX console
//!
//! Re-exports the ZX FFI module from nethercore.

#[path = "../../nethercore/include/zx.rs"]
mod zx;

pub use zx::*;
