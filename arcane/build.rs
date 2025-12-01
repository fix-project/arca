use std::{env, path::PathBuf};

fn main() {
    println!("cargo::rustc-link-arg=-no-pie");

    let prefix = autotools::build("../modules/arca-musl")
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    println!("cargo::rerun-if-changed=../modules/arca-musl");

    eprintln!("{prefix:?}");

    let headers = vec!["a.h"];
    for header in &headers {
        println!("cargo::rerun-if-changed={header}");
    }
    let bindings = bindgen::Builder::default()
        .headers(headers)
        .clang_args(["-nostdinc", "-isystem", &(prefix.clone() + "/include")])
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
