fn main() {
    println!("cargo::rerun-if-changed=src/fixpoint.h");
    println!("cargo::rerun-if-changed=src/fixpoint.c");

    cc::Build::new()
        .file("src/fixpoint.c")
        .include("src")
        .flag("-mreference-types")
        .opt_level(2)
        .compile("fixpoint");
}
