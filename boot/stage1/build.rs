fn main() {
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rustc-link-arg=-Tboot/stage1/link.x");
}
