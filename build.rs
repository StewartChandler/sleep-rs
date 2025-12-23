use embed_manifest::{embed_manifest, manifest::ActiveCodePage, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("EnableUTFEight").active_code_page(ActiveCodePage::Utf8))
            .expect("unable to embed manifest");
    }
    println!("cargo:rustc-link-arg=/ENTRY:_start");
    println!("cargo:rustc-link-arg=/SUBSYSTEM:console");
    println!("cargo:rustc-link-arg=/NODEFAULTLIB");
    // required for some definitions apearently
    println!("cargo:rustc-link-lib=ucrt");
    println!("cargo:rerun-if-changed=build.rs");
}
