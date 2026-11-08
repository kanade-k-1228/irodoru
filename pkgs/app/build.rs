fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=windows/app.rc");
    println!("cargo:rerun-if-changed=icons/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var_os("CARGO_FEATURE_DESKTOP").is_some()
    {
        embed_resource::compile_for("windows/app.rc", ["irodoru"], embed_resource::NONE)
            .manifest_required()
            .expect("Failed to embed the Windows application icon");
    }
}
