fn main() {
    for h in std::fs::read_dir("inc").unwrap().flatten() {
        println!(
            "cargo::rerun-if-changed=inc/{}",
            h.file_name().into_string().unwrap()
        );
    }

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let mut build = cc::Build::new();
    build
        .include("inc")
        .flag("-nostdlib")
        .flag("-ffreestanding")
        .flag("-m32")
        .flag("-march=i586")
        .flag("-mno-sse")
        .flag("-mno-avx");
    for c in std::fs::read_dir("src").unwrap().flatten() {
        let path = format!("src/{}", c.file_name().into_string().unwrap());
        if path.ends_with(".c") || path.ends_with(".S") {
            build.file(&path);
            println!("cargo::rerun-if-changed={}", path);
        }
    }
    println!("cargo::rerun-if-changed=etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{}/etc/memmap.ld", dir);
    println!("cargo::rustc-link-arg=-no-pie");
    build.compile("loader");
}
