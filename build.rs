fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/windows/githly.rc");
    println!("cargo:rerun-if-changed=assets/branding/githly.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let numeric_version = ["MAJOR", "MINOR", "PATCH"]
            .map(|part| std::env::var(format!("CARGO_PKG_VERSION_{part}")).unwrap())
            .join(",");
        std::fs::write(
            out_dir.join("githly-version.h"),
            format!("#define GITHLY_VERSION {numeric_version},0\n#define GITHLY_VERSION_STRING \"{version}\"\n"),
        )
        .expect("failed to generate Windows version metadata");
        // Require the resource compiler so Windows builds cannot silently lose the icon.
        embed_resource::compile_for(
            "assets/windows/githly.rc",
            ["githly"],
            embed_resource::ParamsIncludeDirs([
                std::path::Path::new("assets/branding"),
                out_dir.as_path(),
            ]),
        )
        .manifest_required()
        .expect("failed to embed the Githly Windows resources");
    }

    // Native GPUI controls have deep render/layout calls in debug builds.
    // The MSVC default 1 MiB main-thread stack overflows when rendering the
    // animated TabBar; reserve 8 MiB (pages are committed only as needed).
    if std::env::var("TARGET").is_ok_and(|target| target.ends_with("windows-msvc")) {
        println!("cargo:rustc-link-arg-bin=githly=/STACK:8388608");
    }
}
