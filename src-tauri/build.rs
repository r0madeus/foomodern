use std::path::PathBuf;

fn main() {
    link_libmpv_windows();
    tauri_build::build()
}

/// On Windows, libmpv comes from `src-tauri/lib/` (filled by
/// `scripts/fetch-libmpv.ps1`): `mpv.lib` is the import library used at link
/// time and `libmpv-2.dll` must sit next to every executable we produce
/// (the app, and the test binaries under `deps/`).
/// On Linux the system libmpv (`libmpv-dev`) is used and nothing happens here.
fn link_libmpv_windows() {
    println!("cargo:rerun-if-changed=lib");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let lib_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("lib");
    let import_lib = lib_dir.join("mpv.lib");
    let dll = lib_dir.join("libmpv-2.dll");
    if !import_lib.exists() || !dll.exists() {
        panic!(
            "libmpv not found in {}. Run `powershell -ExecutionPolicy Bypass -File scripts/fetch-libmpv.ps1` from the repository root first.",
            lib_dir.display()
        );
    }
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // OUT_DIR = target/<triple?>/<profile>/build/<pkg>-<hash>/out
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let profile_dir = out_dir.ancestors().nth(3).unwrap().to_path_buf();
    for dir in [profile_dir.clone(), profile_dir.join("deps")] {
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("libmpv-2.dll");
        let stale = match (std::fs::metadata(&target), std::fs::metadata(&dll)) {
            (Ok(t), Ok(s)) => t.len() != s.len(),
            _ => true,
        };
        if stale {
            std::fs::copy(&dll, &target).expect("failed to copy libmpv-2.dll");
        }
    }
}
