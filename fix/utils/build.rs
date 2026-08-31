fn main() {
    println!("cargo::rerun-if-changed=src/utils.h");
    println!("cargo::rerun-if-changed=src/utils.c");

    cc::Build::new()
        .file("src/utils.c")
        .include("src")
        .flag("-mreference-types")
        .opt_level(2)
        .compile("fixutils");
}
