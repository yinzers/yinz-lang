// Build script for ynz-watch — same as ynz-driver/build.rs.
// Locates libynz_runtime.a and emits YNZ_RT_LIB_PATH so write_binary can
// embed the runtime bytes with include_bytes! and link them into compiled programs.
fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = std::path::PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let lib_dir = workspace_root.join("target").join(&profile);

    println!(
        "cargo:rustc-env=YNZ_RT_LIB_PATH={}",
        lib_dir.join("libynz_runtime.a").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        lib_dir.join("libynz_runtime.a").display()
    );
    println!("cargo:rerun-if-env-changed=PROFILE");
}
