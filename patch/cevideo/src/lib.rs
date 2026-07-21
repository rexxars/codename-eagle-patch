//! smackw32.dll - modern video playback shim for Codename Eagle.
//!
//! This crate builds as `smackw32.dll`, a drop-in replacement for the game's
//! Smacker video library. So far it holds the pure, host-testable logic modules
//! (fake Smack context, sidecar paths, pixel packing/blit, frame pacing, handle
//! registry); the decoder, audio, proxy and Windows FFI layer that consume them
//! land in later tasks.

// The modules below are exercised only by their unit tests until the FFI layer
// (later task) calls them, so the plain `cargo build` sees them as unused.
#![allow(dead_code)]

mod audio;
mod bigstack;
mod color;
mod decoder;
mod logging;
mod pacing;
mod pixels;
mod registry;
mod session;
mod sidecar;
mod smack_struct;

// The engine-facing FFI surface and the stock-DLL proxy are Windows-only: they
// use the stdcall ABI and `LoadLibraryA`/`GetProcAddress`. They compile only for
// the `i686-pc-windows-msvc` target; the host build (and its `cargo test`) omits
// them and exercises the logic modules directly.
#[cfg(windows)]
mod ffi;
#[cfg(windows)]
mod proxy;
