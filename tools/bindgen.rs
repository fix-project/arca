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
        "wasm" => builder.ignore_functions(),
        _ => panic!("unknown binding mode"),
    };
    builder
        .generate()
        .expect("generate bindings")
        .write_to_file(&args[3])
        .expect("write bindings");
}
