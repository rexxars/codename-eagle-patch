//! Apply the module-definition file so the stub exports the exact decorated
//! stdcall names of the stock smackw32 DLL. Windows cdylib only.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!("cargo::rustc-link-arg-cdylib=/DEF:{manifest_dir}/smackw32_orig.def");
        println!("cargo::rerun-if-changed=smackw32_orig.def");
    }
}
