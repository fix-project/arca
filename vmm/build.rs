use std::{env, path::PathBuf};

use anyhow::Result;
fn main() -> Result<()> {
    let bindings = bindgen::Builder::default()
        .header("src/vhost_bindings.h")
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .clang_macro_fallback()
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("vhost_bindings.rs"))
        .expect("Couldn't write bindings!");
    Ok(())
}
