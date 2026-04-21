fn main() {
    cc::Build::new().file("src/start.S").compile("start");
    println!("cargo::rerun-if-changed=src/start.S");
    println!("cargo::rustc-link-arg=-no-pie");
    println!("cargo::rerun-if-changed=etc/memmap.ld");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rustc-link-arg=-T{dir}/etc/memmap.ld");

    let prefix = autotools::Config::new("../modules/arca-musl")
        .build()
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    println!("cargo::rerun-if-changed=../modules/arca-musl");
    println!("cargo::rustc-link-search={prefix}/lib");
    println!("cargo::rustc-link-lib=static=c");
}
