#[derive(Debug)]
struct VectorTypes;

impl bindgen::callbacks::ParseCallbacks for VectorTypes {
    fn blocklisted_type_implements_trait(
        &self,
        name: &str,
        derive_trait: bindgen::callbacks::DeriveTrait,
    ) -> Option<bindgen::callbacks::ImplementsTrait> {
        use bindgen::callbacks::{DeriveTrait, ImplementsTrait};
        (name == "u8x32" && matches!(derive_trait, DeriveTrait::Copy | DeriveTrait::Debug))
            .then_some(ImplementsTrait::Yes)
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "bindgen <arca|wasm> <header> <output>");

    let mut builder = bindgen::Builder::default()
        .header(&args[2])
        .use_core()
        // Avoid requiring rustfmt on the host during binding generation.
        .formatter(bindgen::Formatter::None)
        .clang_args(["-ffreestanding"]);
    builder = match args[1].as_str() {
        "arca" => builder.default_enum_style(bindgen::EnumVariation::ModuleConsts),
        "wasm" => builder
            .ignore_functions()
            .blocklist_type("u8x32")
            .parse_callbacks(Box::new(VectorTypes))
            .raw_line("pub type u8x32 = core::simd::u8x32;"),
        _ => panic!("unknown binding mode"),
    };
    builder
        .generate()
        .expect("generate bindings")
        .write_to_file(&args[3])
        .expect("write bindings");
}
