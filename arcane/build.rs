use std::{env, path::PathBuf};

fn main() {
    println!("cargo::rustc-link-arg=-no-pie");

    cc::Build::new()
        .compiler("clang")
        .file("src/syscalls.c")
        .include("inc")
        .compile("syscalls");

    println!("cargo::rerun-if-changed=src/syscalls.c");

    let headers = vec!["inc/arca.h"];

    for header in &headers {
        println!("cargo::rerun-if-changed={header}");
    }

    let bindings = bindgen::Builder::default()
        .headers(headers)
        .clang_args(["-ffreestanding"])
        .use_core()
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
