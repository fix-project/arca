use std::{env, path::PathBuf};

fn main() {
    println!("cargo::rustc-link-arg=-no-pie");
    let bindings = bindgen::Builder::default()
        .header("syscall.h")
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
