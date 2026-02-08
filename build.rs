fn main() {
    println!("cargo:rustc-link-arg=/ENTRY:_start");
    println!("cargo:rustc-link-arg=/SUBSYSTEM:console");
    println!("cargo:rustc-link-arg=/NODEFAULTLIB");
    // required for some definitions apearently
    println!("cargo:rustc-link-lib=ucrt");
    println!("cargo:rerun-if-changed=build.rs");
}
