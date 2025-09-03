use std::env;

fn main() {
    cc::Build::new().file("src/start.S").compile("start");
    println!("cargo::rerun-if-changed=src/start.S");
    println!("cargo::rustc-link-arg=-no-pie");
    println!("cargo::rerun-if-changed=etc/memmap.ld");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rustc-link-arg=-T{dir}/etc/memmap.ld");

    let prefix = env::var("ARCA_SDK").unwrap_or("/opt/arca/musl".to_string());
    println!("cargo::rustc-link-search={prefix}/lib");
    println!("cargo::rustc-link-lib=static=c");
}
