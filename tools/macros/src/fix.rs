use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, Ident, ItemFn, Pat, parse_macro_input};

pub fn apply(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let callback = &item.sig.ident;
    quote! {
        #item
        #[unsafe(export_name = "_fix_apply_inner")]
        pub extern "C" fn __fix_apply() -> u32 {
            match ::fix::__apply(|combination| ::fix::__finish(#callback(combination)?)) {
                Ok(()) => 0,
                Err(error) => error as u32,
            }
        }
    }
    .into()
}

pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let context = if attr.is_empty() {
        None
    } else {
        Some(parse_macro_input!(attr as Ident))
    };
    let item = parse_macro_input!(item as ItemFn);
    let callback = &item.sig.ident;
    let mut bindings = Vec::new();
    let mut arguments = Vec::new();
    for (index, input) in item.sig.inputs.iter().enumerate() {
        let FnArg::Typed(input) = input else {
            return syn::Error::new_spanned(input, "expected a function argument")
                .to_compile_error()
                .into();
        };
        if index == 0 && context.is_some() {
            if !matches!(input.pat.as_ref(), Pat::Ident(name) if Some(&name.ident) == context.as_ref())
            {
                return syn::Error::new_spanned(input, "the context must be the first parameter")
                    .to_compile_error()
                    .into();
            }
            arguments.push(quote!(combination));
        } else {
            let name = format_ident!("argument_{index}");
            let ty = &input.ty;
            bindings.push(quote!(let #name: #ty = ::fix::Argument::from_argument(arguments.next().unwrap())?;));
            arguments.push(quote!(#name));
        }
    }
    let count = bindings.len();
    quote! {
        #item
        #[::fix::apply]
        fn __fix_main(combination: &::fix::Handle<::fix::Object<::fix::Tree>, ::fix::Focus>)
            -> Result<::fix::Handle<::fix::Any, ::fix::Capture>, ::fix::Error>
        {
            let result = (|| {
                let Some((arguments, count)) = ::fix::fixed_arguments::<#count>(combination)? else {
                    return ::fix::Value::capture(combination);
                };
                let mut arguments = arguments.into_iter();
                #(#bindings)*
                let result = #callback(#(#arguments),*);
                if count == #count {
                    ::fix::Value::capture(&result)
                } else {
                    ::fix::apply_excess(combination, result, #count)
                }
            })();
            result.or_else(::fix::error)
        }
    }
    .into()
}
