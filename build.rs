fn main() {
    println!("cargo:rustc-link-arg=/ENTRY:_start");
    println!("cargo:rustc-link-arg=/SUBSYSTEM:console");
    println!("cargo:rustc-link-arg=/NODEFAULTLIB");
}
