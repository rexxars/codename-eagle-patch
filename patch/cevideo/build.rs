//! Build script: on Windows, re-export the shim's plain `#[no_mangle]` symbols
//! under the engine's exact decorated stdcall names via `smackw32.def`.
//!
//! `#[export_name]` can't emit `_SmackOpen@12` on i686-pc-windows-msvc (the
//! linker re-decorates a stdcall export_name into `__SmackOpen@12@12`), so we
//! feed a module-definition file to the linker instead. Scoped to the cdylib so
//! integration-test executables aren't affected, and to Windows targets so the
//! dev-host build (a different linker with no `/DEF`) is untouched.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is always set for build scripts");
        println!("cargo::rustc-link-arg-cdylib=/DEF:{manifest_dir}/smackw32.def");
        println!("cargo::rerun-if-changed=smackw32.def");
    }
}
