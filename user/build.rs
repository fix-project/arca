fn main() {
    cc::Build::new().file("src/start.S").compile("start");
    println!("cargo::rerun-if-changed=src/start.S");
    println!("cargo::rustc-link-arg=-no-pie");
    println!("cargo::rerun-if-changed=etc/memmap.ld");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rustc-link-arg=-T{}/etc/memmap.ld", dir);
}
