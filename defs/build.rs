use std::{env, path::PathBuf};

fn main() {
    println!("cargo::rustc-link-arg=-no-pie");
    let mut headers = vec!["arca/defs.h"];
    let syscalls = env::var("CARGO_FEATURE_SYSCALLS").is_ok();
    if syscalls {
        headers.push("arca/syscall.h");
    }
    for header in &headers {
        println!("cargo::rerun-if-changed={header}");
    }
    let bindings = bindgen::Builder::default()
        .headers(headers)
        .use_core()
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");
    if syscalls {
        cc::Build::new()
            .file("src/syscall.c")
            .file("src/syscall.S")
            .include("arca")
            .compile("syscall");
        println!("cargo::rerun-if-changed=src/syscall.c");
    }

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
