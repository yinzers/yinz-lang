// Build script for ynz-driver.
//
// Locates the `libynz_runtime.a` static library produced by the `ynz-runtime` crate
// and emits the linker flags so that the `cc` invocation in `src/build.rs` can find it.
//
// The path changes based on the cargo profile (debug vs release), so we compute it
// dynamically from CARGO_MANIFEST_DIR and the OUT_DIR / PROFILE env vars.

fn main() {
    // CARGO_MANIFEST_DIR is the directory of this Cargo.toml (crates/ynz-driver/).
    // The workspace target directory is two levels up.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = std::path::PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // The profile used for building ynz-runtime defaults to the same profile as the driver.
    // PROFILE is set by cargo to "debug" or "release".
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let lib_dir = workspace_root.join("target").join(&profile);

    // Emit the path so the driver binary can find the archive at compile time.
    // The driver's build.rs (this file) runs before the driver source is compiled,
    // so env!("YNZ_RT_LIB_DIR") and env!("YNZ_RT_LIB_NAME") will be available.
    println!("cargo:rustc-env=YNZ_RT_LIB_DIR={}", lib_dir.display());
    println!("cargo:rustc-env=YNZ_RT_LIB_NAME=ynz_runtime");

    // Re-run this build script when the runtime library changes.
    println!(
        "cargo:rerun-if-changed={}",
        lib_dir.join("libynz_runtime.a").display()
    );
    println!("cargo:rerun-if-env-changed=PROFILE");
}
