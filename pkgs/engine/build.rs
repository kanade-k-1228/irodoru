fn main() {
    println!("cargo:rerun-if-env-changed=IRODORU_CPU_ORT_LIB_DIR");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // alternative-backend disables ort-sys linking: our loader selects either
    // this API or the API exported by the registered Windows ML package.
    if let Some(directory) = std::env::var_os("IRODORU_CPU_ORT_LIB_DIR") {
        let directory = std::path::PathBuf::from(directory);
        assert!(
            directory.join("irodoru_ort_cpu.lib").is_file(),
            "Run scripts/build-cpu-ort.ps1 first"
        );
        println!("cargo:rustc-link-search=native={}", directory.display());
    }
    // No env var is necessary for cargo check; a real link must resolve this
    // library (and fails rather than shipping a build without CPU fallback).
    println!("cargo:rustc-link-lib=static=irodoru_ort_cpu");
    for library in ["shell32", "advapi32", "ole32", "shlwapi", "userenv"] {
        println!("cargo:rustc-link-lib={library}");
    }
}
