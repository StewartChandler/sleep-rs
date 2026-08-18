fn main() {
    println!("cargo:rustc-link-arg=/ENTRY:_start");
    println!("cargo:rustc-link-arg=/SUBSYSTEM:console");
    println!("cargo:rustc-link-arg=/NODEFAULTLIB");
    // able to get arround this by providing definitions, shouldn't make much a difference
    // // required for some definitions apearently
    // println!("cargo:rustc-link-lib=ucrt");
    println!("cargo:rerun-if-changed=build.rs");
}
